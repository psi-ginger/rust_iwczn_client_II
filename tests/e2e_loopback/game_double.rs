// Step 6.2 — the fake game service (WO-6.2 §4, §5.1).
//
// Speaks the full game conversation byte-exactly over loopback: the
// unsolicited opcode-1 handshake on accept (link-ciphered — the client's
// inbound decoder is keyed `0x12/0x34` at construct, `entry.rs:396-400`),
// the opcode-2 game login, the 15-slot roster, select-char, SERVERINFO +
// the opcode-5 world-entry record, the village round-trip, the in-world
// dispatcher set, heartbeat observation, then a server-side FIN that must
// surface as a clean teardown. Client→server frames are decoded the way the
// legacy service does (`Cipher::session_cipher(key1, key2)`, `main.rs:704`)
// and asserted byte-for-byte against the independent constants in
// `frames.rs`.
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

use goonzu_client::net::cipher::Cipher;

use crate::frames::{
    char_update_body, chat_body, encode_link, frame, game_login_payload, handshake_payload,
    logout_payload, make_char_record, person_ip_body, read_ciphered, roster_entry,
    select_char_payload, server_info_payload, village_request_payload, village_struct_payload, Dir,
    Leg, Mutation, ReadOutcome, Transcript, CHAR_UNIQUE, GAME_KEY1, GAME_KEY2, HEARTBEAT_WIRE,
    MAKE_CHAR_PAYLOAD_LEN, NEARBY_UNIQUE, OCCUPIED_SLOT, OP_CHARLIST, OP_CHARUPDATE, OP_CHAT,
    OP_CLIENTCHARUNIQUE, OP_GAME_LOGIN, OP_HEARTBEAT, OP_LOGINMANY, OP_LOGOUT, OP_MAKE_CHAR,
    OP_PERSON_IP, OP_SELECTCHAR, OP_SERVERINFO, OP_VILLAGESTRUCT_REQ, OP_VILLAGESTRUCT_RES,
    ROSTER_SLOTS, VILLAGE_REF,
};

/// What the game double observed by the time its conversation ended.
#[derive(Debug)]
pub(crate) struct GameReport {
    /// How many complete client frames arrived (the happy path sees 4:
    /// game login, select-char, village request, then the heartbeats).
    pub(crate) frames_received: usize,
    /// How many server frames were written (the happy path writes 24: the
    /// handshake, the login ack is client-side, 15 roster frames,
    /// SERVERINFO, the opcode-5 record, the village answer, the five
    /// in-world frames).
    pub(crate) frames_sent: usize,
    /// How many heartbeat frames were observed in-world. The happy path
    /// requires at least one (WO-6.2 §5.1); mutation paths stop earlier.
    pub(crate) heartbeats: usize,
    /// Whether the client closed before the conversation's next step (every
    /// mutation path ends this way: the driver refuses, drops the link).
    pub(crate) closed_early: bool,
    /// Whether a frame arrived short or with a failed integrity check.
    pub(crate) saw_corruption: bool,
}

/// Runs the game conversation against one accepted connection.
///
/// # Panics
///
/// On any byte-exactness violation (the assertion *is* the test) and on any
/// socket failure — every peer here is this test's own loopback listener.
pub(crate) async fn run_game_double(
    listener: TcpListener,
    account: &str,
    password: &str,
    mutation: Mutation,
    transcript: &Transcript,
) -> GameReport {
    let (mut stream, _peer) = listener.accept().await.expect("game accept");

    // One persistent link cipher for every server→client frame: its keys
    // are the counter and tick once per call (`cipher.rs:23-30`), so a
    // single instance stays in lockstep with the client's inbound decoder.
    let mut link = Cipher::link_cipher();
    // One persistent session cipher for every client→server frame — the
    // seeds the handshake below just handed out.
    let mut session = Cipher::session_cipher(GAME_KEY1, GAME_KEY2);

    // 1. The unsolicited GAMEMSG_RESPONSE_CLIENTCHARUNIQUE: SI32
    // char_unique + two session keys + version (`entry.rs:38-60`).
    let handshake = frame(OP_CLIENTCHARUNIQUE, &handshake_payload());
    let wire = encode_link(&mut link, &handshake);
    stream.write_all(&wire).await.expect("write handshake");
    transcript.push(Leg::Game, Dir::S2C, handshake.clone(), wire.len());

    let mut report = GameReport {
        frames_received: 0,
        frames_sent: 1,
        heartbeats: 0,
        closed_early: false,
        saw_corruption: false,
    };

    // 2. GAMEMSG_REQUEST_LOGIN — `usSize` 53, the 48-byte cltLogInInfo
    // carrying the front's auth_key and the canonical userid
    // (`game/login.rs:59-65,196-221`).
    match read_ciphered(&mut stream, &mut session).await {
        ReadOutcome::Frame { wire, plain } => {
            assert_eq!(
                plain,
                frame(OP_GAME_LOGIN, &game_login_payload(password)),
                "the game login must carry siAuthKey = the front's token at \
                 0..4 and the response's canonical userid at 4..24 \
                 (CommonHeader.h:1442-1452, MOD-04)"
            );
            transcript.push(Leg::Game, Dir::C2S, plain, wire.len());
            report.frames_received += 1;
        }
        outcome => return early_stop(report, outcome),
    }

    // 3. The roster: exactly CHAR_SLOTS opcode-3 frames, slot
    // OCCUPIED_SLOT carrying a name (`char_info.rs:83-95`,
    // `driver.rs:753-771`). The mutation empties the occupied slot's name.
    for slot in 0..ROSTER_SLOTS {
        let name = if slot as i32 == OCCUPIED_SLOT {
            Some("Hero")
        } else {
            None
        };
        let mut entry = roster_entry(slot as u8, account, name);
        if mutation == Mutation::RosterName && slot as i32 == OCCUPIED_SLOT {
            entry[80] = 0; // szName[0] -> NUL: the slot now reads empty
        }
        let plain = frame(OP_CHARLIST, &entry);
        let wire = encode_link(&mut link, &plain);
        stream.write_all(&wire).await.expect("write roster frame");
        transcript.push(Leg::Game, Dir::S2C, plain, wire.len());
        report.frames_sent += 1;
    }

    // 4. GAMEMSG_REQUEST_SELECTCHAR — bare SI32 slot (`NSelectChar.cpp:1353`).
    match read_ciphered(&mut stream, &mut session).await {
        ReadOutcome::Frame { wire, plain } => {
            assert_eq!(
                plain,
                frame(OP_SELECTCHAR, &select_char_payload(OCCUPIED_SLOT)),
                "SELECTCHAR must carry the occupied slot as a bare SI32 \
                 (world_entry.rs:139-147)"
            );
            transcript.push(Leg::Game, Dir::C2S, plain, wire.len());
            report.frames_received += 1;
        }
        outcome => return early_stop(report, outcome),
    }

    // 5. RESPONSE_SERVERINFO — 48 bytes, siServiceArea = 0x08
    // (`Server-Login.cpp:184`, `world_entry.rs:154-197`).
    let info = frame(OP_SERVERINFO, &server_info_payload());
    let wire = encode_link(&mut link, &info);
    stream.write_all(&wire).await.expect("write SERVERINFO");
    transcript.push(Leg::Game, Dir::S2C, info.clone(), wire.len());
    report.frames_sent += 1;

    // 6. RESPONSE_LOGIN — the 592-byte clMakeCharInfo whose
    // uiMakeCharUnique must equal the handshake's char_unique
    // (`Client-Login.cpp:90-116`, `world_entry.rs:738-756`). The mutation
    // flips that unique by one.
    let mut record = make_char_record(CHAR_UNIQUE as u16, 12);
    if mutation == Mutation::MakeCharUnique {
        record[4] ^= 0x01; // uiMakeCharUnique low byte: 4242 -> 4243
    }
    let record = frame(OP_MAKE_CHAR, &record);
    let wire = encode_link(&mut link, &record);
    stream.write_all(&wire).await.expect("write opcode 5");
    transcript.push(Leg::Game, Dir::S2C, record.clone(), wire.len());
    report.frames_sent += 1;

    // 7. GAMEMSG_REQUEST_VILLAGESTRUCT — bare SI32 ref
    // (`Client-Login.cpp:281`).
    match read_ciphered(&mut stream, &mut session).await {
        ReadOutcome::Frame { wire, plain } => {
            assert_eq!(
                plain,
                frame(OP_VILLAGESTRUCT_REQ, &village_request_payload(VILLAGE_REF)),
                "the village request must carry the configured ref as a bare \
                 SI32 (world_entry.rs:147-152)"
            );
            transcript.push(Leg::Game, Dir::C2S, plain, wire.len());
            report.frames_received += 1;
        }
        outcome => return early_stop(report, outcome),
    }

    // 8. RESPONSE_VILLAGESTRUCT — 22 bytes, non-negative village
    // (`Client-SystemNPC.cpp:875`, `world_entry.rs:552-566`).
    let village = frame(OP_VILLAGESTRUCT_RES, &village_struct_payload(VILLAGE_REF));
    let wire = encode_link(&mut link, &village);
    stream.write_all(&wire).await.expect("write village struct");
    transcript.push(Leg::Game, Dir::S2C, village.clone(), wire.len());
    report.frames_sent += 1;

    // 9. The in-world set: 6 (two PC records), 11, 118, 200, 215
    // (`dispatcher.rs:16-28`). The chat mutation lands on the 118 body.
    let mut body = Vec::with_capacity(2 * MAKE_CHAR_PAYLOAD_LEN);
    body.extend_from_slice(&make_char_record(CHAR_UNIQUE as u16, 12));
    body.extend_from_slice(&make_char_record(NEARBY_UNIQUE as u16, 9));
    for (cmd, payload) in [
        (OP_LOGINMANY, body),
        (OP_LOGOUT, logout_payload(NEARBY_UNIQUE).to_vec()),
        (
            OP_CHAT,
            chat_body(NEARBY_UNIQUE as i16, "someone", "hi all"),
        ),
        (OP_CHARUPDATE, char_update_body(CHAR_UNIQUE as i16).to_vec()),
        (OP_PERSON_IP, person_ip_body(CHAR_UNIQUE as i16).to_vec()),
    ] {
        let payload = if mutation == Mutation::ChatDataSize && cmd == OP_CHAT {
            // Declared siDataSize off by one from the bytes present — the
            // strict size gate must refuse (`dispatcher/tests.rs:216-241`).
            let mut payload = payload;
            let declared = i16::from_le_bytes([payload[0], payload[1]]);
            payload[0..2].copy_from_slice(&(declared + 1).to_le_bytes());
            payload
        } else {
            payload
        };
        let plain = frame(cmd, &payload);
        let wire = encode_link(&mut link, &plain);
        stream.write_all(&wire).await.expect("write in-world frame");
        transcript.push(Leg::Game, Dir::S2C, plain, wire.len());
        report.frames_sent += 1;
    }

    // 10. In-world: the heartbeat is the only frame this driver sends
    // (`heartbeat.rs`, opcode 100, header-only; the in-world loop has no
    // other outbound path — `driver.rs:676-687`). One read observes it (or
    // the peer's departure on a refusal path); anything else is a protocol
    // violation and the assertions below fail loudly. The double then ends
    // the session with a server-side FIN.
    match read_ciphered(&mut stream, &mut session).await {
        ReadOutcome::Frame { wire, plain } => {
            assert_eq!(
                u16::from_le_bytes([plain[3], plain[4]]),
                OP_HEARTBEAT,
                "in-world client frames must be heartbeats (opcode 100); \
                 got opcode {}",
                u16::from_le_bytes([plain[3], plain[4]])
            );
            assert_eq!(
                plain.as_slice(),
                &HEARTBEAT_WIRE,
                "the heartbeat must be the header-only image \
                 [usSize=5][ucChk=0][usCmd=100] (heartbeat/tests.rs:74)"
            );
            transcript.push(Leg::Game, Dir::C2S, plain, wire.len());
            report.frames_received += 1;
            report.heartbeats += 1;
        }
        ReadOutcome::Eof => report.closed_early = true,
        ReadOutcome::Corrupt => report.saw_corruption = true,
    }

    // 11. The server-side close: dropping the stream sends FIN, which the
    // driver's in-world loop must surface as Cause::ServerDisconnect and a
    // clean teardown (`event_loop.rs:331-333`, WO-6.2 §5.4).
    drop(stream);
    report
}

/// Records a conversation that ended before its last step (every mutation
/// path, plus any protocol failure the client itself refuses) and hands the
/// report back — the doubles never hang on a peer that already left.
fn early_stop(mut report: GameReport, outcome: ReadOutcome) -> GameReport {
    match outcome {
        ReadOutcome::Eof => report.closed_early = true,
        ReadOutcome::Corrupt => report.saw_corruption = true,
        ReadOutcome::Frame { .. } => {
            // A frame arrived after the point the conversation should have
            // ended; the transcript assertion in the root reports it.
        }
    }
    report
}
