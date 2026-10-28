// Step 6.2 — the fake front service (WO-6.2 §4, §5.1).
//
// Speaks the full front conversation byte-exactly over loopback: the
// unsolicited SECRETKEY handshake on accept, then notice, world list and
// login. Every client→server frame is decoded the way the legacy service
// does (`GetEncrypt()->Decode`, `FrontClientProtocol.cpp:85`) and asserted
// byte-for-byte against the independent constants in `frames.rs` — never
// against anything the client's own encoder produced
// (`CONTRIBUTING_PHASE2.md:74-76`).
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

use goonzu_client::net::cipher::Cipher;

use crate::frames::{
    account_id_payload, frame, login_request_payload, login_response_payload, read_ciphered,
    worldinfo_entry, Dir, Leg, Mutation, ReadOutcome, Transcript, FRONT_KEY1, FRONT_KEY2,
    NOTICE_TEXT, OP_REQUEST_LOGIN, OP_REQUEST_NOTICE, OP_REQUEST_WORLDLIST, OP_RESPONSE_LOGIN,
    OP_RESPONSE_NOTICE, OP_RESPONSE_WORLDLIST, SECRETKEY_WIRE,
};

/// What the front double observed by the time its conversation ended.
#[derive(Debug)]
pub(crate) struct FrontReport {
    /// How many complete client frames arrived before the peer closed (or
    /// the conversation broke): the happy path sees exactly 3 (notice,
    /// world list, login).
    pub(crate) frames_received: usize,
    /// How many server frames were written: the happy path writes 4
    /// (SECRETKEY, notice, world list, login response).
    pub(crate) frames_sent: usize,
    /// Whether the client closed the connection cleanly after the login
    /// response — the driver drops the front link before dialling the game
    /// service (`driver.rs:574-576`).
    pub(crate) closed_cleanly: bool,
    /// Whether the conversation ended because the client left *before* its
    /// next expected step — what every refusal path looks like from here.
    pub(crate) ended_by_peer: bool,
    /// Whether a frame arrived short or with a failed integrity check.
    pub(crate) saw_corruption: bool,
}

/// Runs the front conversation against one accepted connection.
///
/// # Panics
///
/// On any byte-exactness violation (the assertion *is* the test) and on any
/// socket failure — every peer here is this test's own loopback listener.
pub(crate) async fn run_front_double(
    listener: TcpListener,
    account: &str,
    password: &str,
    mac: &str,
    mutation: Mutation,
    transcript: &Transcript,
) -> FrontReport {
    let (mut stream, _peer) = listener.accept().await.expect("front accept");

    // The unsolicited FRONTMSG_RESPONSE_SECRETKEY, in the clear: the front
    // link's inbound half is plaintext, and the frame arrives before any
    // client request (`front.rs:5-6`, `world_list/tests.rs:19-22`).
    stream
        .write_all(&SECRETKEY_WIRE)
        .await
        .expect("write SECRETKEY");
    transcript.push(
        Leg::Front,
        Dir::S2C,
        SECRETKEY_WIRE.to_vec(),
        SECRETKEY_WIRE.len(),
    );

    // The seeds travel in that frame, so the service decodes every inbound
    // frame with the session cipher it just negotiated.
    let mut cipher = Cipher::session_cipher(FRONT_KEY1, FRONT_KEY2);

    let mut report = FrontReport {
        frames_received: 0,
        frames_sent: 1,
        closed_cleanly: false,
        ended_by_peer: false,
        saw_corruption: false,
    };

    // 1. REQUEST_NOTICE — header-only, `usSize` 5 (`FrontMsg.h:477-480`).
    match read_ciphered(&mut stream, &mut cipher).await {
        ReadOutcome::Frame { wire, plain } => {
            assert_eq!(
                plain,
                frame(OP_REQUEST_NOTICE, &[]),
                "the client's REQUEST_NOTICE must be [usSize=5][ucChk][usCmd=2] \
                 byte-exact (FrontMsg.h:45,477-480)"
            );
            transcript.push(Leg::Front, Dir::C2S, plain, wire.len());
            report.frames_received += 1;
        }
        outcome => return early_stop(report, outcome),
    }

    // 2. RESPONSE_NOTICE — 16 bytes of C-string text
    // (`FrontClientProtocol.cpp:760`, `world_list.rs:162-166`).
    let notice = frame(OP_RESPONSE_NOTICE, &NOTICE_TEXT);
    stream.write_all(&notice).await.expect("write notice");
    transcript.push(Leg::Front, Dir::S2C, notice.clone(), notice.len());
    report.frames_sent += 1;

    // 3. REQUEST_WORLDLIST — `usSize` 25, `szAccountID` NUL-padded
    // (`FrontMsg.h:486-491`).
    match read_ciphered(&mut stream, &mut cipher).await {
        ReadOutcome::Frame { wire, plain } => {
            assert_eq!(
                plain,
                frame(OP_REQUEST_WORLDLIST, &account_id_payload(account)),
                "the world-list request must carry the configured account id \
                 in szAccountID[20], NUL-padded (FrontMsg.h:486-491)"
            );
            transcript.push(Leg::Front, Dir::C2S, plain, wire.len());
            report.frames_received += 1;
        }
        outcome => return early_stop(report, outcome),
    }

    // 4. RESPONSE_WORLDLIST — `usCount` + one 428-byte WORLDINFO
    // (`WorldList.cpp:215-256`, `world_list.rs:69-91`), optionally carrying
    // the mutation's flipped byte.
    let world = match mutation {
        Mutation::WorldListCount => {
            // usCount declared as 2 while one entry follows — Truncated
            // (`world_list.rs:194-200`).
            let mut payload = vec![0x02, 0x00];
            payload.extend_from_slice(&worldinfo_entry());
            frame(OP_RESPONSE_WORLDLIST, &payload)
        }
        _ => {
            let mut payload = vec![0x01, 0x00];
            payload.extend_from_slice(&worldinfo_entry());
            frame(OP_RESPONSE_WORLDLIST, &payload)
        }
    };
    stream.write_all(&world).await.expect("write world list");
    transcript.push(Leg::Front, Dir::S2C, world.clone(), world.len());
    report.frames_sent += 1;

    // 5. REQUEST_LOGIN — `usSize` 65, the three 20-byte credential slots
    // (`FrontMsg.h:336-348`).
    match read_ciphered(&mut stream, &mut cipher).await {
        ReadOutcome::Frame { wire, plain } => {
            assert_eq!(
                plain,
                frame(
                    OP_REQUEST_LOGIN,
                    &login_request_payload(account, password, mac)
                ),
                "the login request must carry userid/userpw/macaddress at \
                 0/20/40, each NUL-padded in 20 bytes (FrontMsg.h:336-348)"
            );
            transcript.push(Leg::Front, Dir::C2S, plain, wire.len());
            report.frames_received += 1;
        }
        outcome => return early_stop(report, outcome),
    }

    // 6. RESPONSE_LOGIN — 108 bytes, `usResult` 0, the auth_key and the
    // canonical userid the game login must then carry
    // (`FrontMsg.h:419-442`).
    let login = frame(OP_RESPONSE_LOGIN, &login_response_payload());
    stream
        .write_all(&login)
        .await
        .expect("write login response");
    transcript.push(Leg::Front, Dir::S2C, login.clone(), login.len());
    report.frames_sent += 1;

    // 7. The client drops the front connection before dialling the game
    // service — a clean EOF is what closes this leg.
    match read_ciphered(&mut stream, &mut cipher).await {
        ReadOutcome::Eof => report.closed_cleanly = true,
        outcome => return early_stop(report, outcome),
    }
    report
}

/// Records a conversation that ended before its last step (every mutation
/// path, plus any protocol failure the client itself refuses) and hands the
/// report back — the doubles never hang on a peer that already left.
fn early_stop(mut report: FrontReport, outcome: ReadOutcome) -> FrontReport {
    match outcome {
        ReadOutcome::Eof => report.ended_by_peer = true,
        ReadOutcome::Corrupt => report.saw_corruption = true,
        ReadOutcome::Frame { .. } => {
            // A frame arrived after the point the conversation should have
            // ended; the transcript assertion in the root reports it.
        }
    }
    report
}
