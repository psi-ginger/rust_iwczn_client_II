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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::time::{interval, MissedTickBehavior};

use goonzu_client::net::cipher::Cipher;

use crate::frames::{
    char_update_body, chat_body, encode_link, frame, game_login_payload, handshake_payload,
    loginmany_body, loginmany_npc_led_body, logout_payload, make_char_record, person_ip_body,
    read_ciphered, roster_entry, select_char_payload, server_info_payload, village_request_payload,
    village_struct_payload, Dir, Leg, Mutation, ReadOutcome, Transcript, CHAR_UNIQUE, GAME_KEY1,
    GAME_KEY2, HEARTBEAT_WIRE, NEARBY_UNIQUE, OCCUPIED_SLOT, OP_CHARLIST, OP_CHARUPDATE, OP_CHAT,
    OP_CLIENTCHARUNIQUE, OP_GAME_LOGIN, OP_HEARTBEAT, OP_LOGINMANY, OP_LOGOUT, OP_MAKE_CHAR,
    OP_PERSON_IP, OP_SELECTCHAR, OP_SERVERINFO, OP_UNHANDLED_IN_WORLD, OP_VILLAGESTRUCT_REQ,
    OP_VILLAGESTRUCT_RES, ROSTER_SLOTS, VILLAGE_REF,
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
    /// Step 6.3: how many dispatcher frames the storm tail wrote beyond the
    /// in-world set (0 for every other tail).
    pub(crate) storm_frames: usize,
    /// Step 6.3: when each byte-exact heartbeat was observed — the cadence
    /// surface the soak asserts (WO-6.3 §6(a)).
    pub(crate) heartbeat_times: Vec<Instant>,
    /// Step 6.3: when the tail sent its server-side FIN (`None` unless the
    /// tail FINs: the storm and the 6.2 default do, the Ctrl+C seam never
    /// does — the client's own close ends that session).
    pub(crate) fin_instant: Option<Instant>,
    /// Step 6.3: whether the client's own close (EOF on the double's read
    /// half) was observed — the teardown-close proxy every 6.3 path asserts
    /// (a driver that never tore down never closes, so EOF proves the close
    /// phase of `terminate` ran, `teardown.rs:366`).
    pub(crate) client_closed: bool,
}

/// Step 6.3: what the double does after the in-world set (WO-6.3 §4-§6).
///
/// The default [`Tail::HeartbeatThenFin`] is the Step 6.2 step-10/11
/// behavior, unchanged; every other variant replaces only that tail.
pub(crate) enum Tail {
    /// Step 6.2, verbatim: read one byte-exact heartbeat, then FIN
    /// (`event_loop.rs` surfaces the close as `Cause::ServerDisconnect`).
    HeartbeatThenFin,
    /// The sustained storm: pace `target_frames` dispatcher frames at a
    /// fixed 100 ms tick while a concurrent reader counts byte-exact
    /// heartbeats; once both the frame target and
    /// [`STORM_MIN_HEARTBEATS`] are met, FIN and drain to the client's
    /// close (WO-6.3 §6(a)).
    Storm {
        /// How many frames the pacing loop must write before it may FIN.
        target_frames: usize,
    },
    /// Write this wire image **verbatim** (no cipher — a malformed frame is
    /// what a broken peer actually puts on the wire) right after the
    /// in-world set, then drain to the client's close (R1).
    RawFault(Vec<u8>),
    /// Encode one valid frame with the link cipher, flip its **last** wire
    /// byte — the integrity tag is computed against the last plaintext byte
    /// (`cipher.rs:279-295`), so only a tag-side flip trips the check —
    /// write it, then drain to the client's close (R2).
    CorruptFault,
    /// Write the first three wire bytes of a valid frame (a valid `usSize`,
    /// an incomplete body) then FIN: the client must discard the partial at
    /// EOF and tear down **cleanly** — the documented behavior this pins
    /// (`game/entry.rs:472-476`, R3).
    PartialFault,
    /// Wait out the first byte-exact heartbeat (proof the in-world loop is
    /// live, so the Ctrl+C claim is deterministic), notify the test's seam,
    /// then drain to the client's close (R4).
    CtrlCSeam(Arc<Notify>),
}

/// How many frames one storm tick writes for a given target: the whole
/// target spreads over 300 ticks (30 s at [`STORM_PACE`]), so the stream
/// rate — not the target — is the fixed dimension and storm-size
/// independence is a falsifiable claim (WO-6.3 §6(a)).
const STORM_TICKS: usize = 300;
/// One storm tick: 100 ms — the doubles' outbound rate is freely tunable
/// (WO-6.3 §3, the scaled-cadence dimension), while the client's own
/// heartbeat ticker stays at its fixed 10 s (`heartbeat.rs:169`).
const STORM_PACE: std::time::Duration = std::time::Duration::from_millis(100);
/// How many byte-exact heartbeats a storm must observe before it may FIN:
/// three full 10 s ticker periods, enough to assert cadence stability
/// across both storm sizes (WO-6.3 §6(a)).
const STORM_MIN_HEARTBEATS: usize = 3;

/// Runs the game conversation against one accepted connection with the
/// in-world tail chosen by the caller (WO-6.3 §4): everything up to step 9
/// is the shared byte-exact conversation — the Step 6.2 flow, unchanged —
/// and only the in-world tail varies ([`Tail`]).
///
/// # Panics
///
/// On any byte-exactness violation (the assertion *is* the test) and on any
/// socket failure — every peer here is this test's own loopback listener.
pub(crate) async fn run_game_double_with(
    listener: TcpListener,
    account: &str,
    password: &str,
    mutation: Mutation,
    transcript: &Transcript,
    tail: Tail,
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
        storm_frames: 0,
        heartbeat_times: Vec::new(),
        fin_instant: None,
        client_closed: false,
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

    // 9. The in-world set: 6 (mixed NPC + PC batch — one NPC record, the
    // live broadcast's shape, then the two PC records), 11, 118, 200, 215
    // (`dispatcher.rs:16-28`). The chat mutation lands on the 118 body.
    for (cmd, payload) in [
        (OP_LOGINMANY, loginmany_body()),
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

    // 10/11. In-world tail (Step 6.3, WO-6.3 §4): the default tail is the
    // Step 6.2 behavior — one heartbeat observed, then a server-side FIN —
    // unchanged; the storm and fault tails take the stream, the ciphers and
    // the report and finish the conversation themselves.
    match tail {
        Tail::HeartbeatThenFin => {
            // The heartbeat is the only frame this driver sends
            // (`heartbeat.rs`, opcode 100, header-only; the in-world loop
            // has no other outbound path — `driver.rs:676-687`). One read
            // observes it (or the peer's departure on a refusal path);
            // anything else is a protocol violation and the assertions
            // fail loudly.
            match read_ciphered(&mut stream, &mut session).await {
                ReadOutcome::Frame { wire, plain } => {
                    assert_heartbeat(&plain);
                    transcript.push(Leg::Game, Dir::C2S, plain, wire.len());
                    report.frames_received += 1;
                    report.heartbeats += 1;
                }
                ReadOutcome::Eof => report.closed_early = true,
                ReadOutcome::Corrupt => report.saw_corruption = true,
            }
            // The server-side close: dropping the stream sends FIN, which
            // the driver's in-world loop must surface as
            // Cause::ServerDisconnect and a clean teardown
            // (`event_loop.rs:331-333`, WO-6.2 §5.4).
            drop(stream);
        }
        Tail::Storm { target_frames } => {
            storm_tail(stream, link, session, target_frames, &mut report).await;
        }
        Tail::RawFault(wire) => {
            raw_fault_tail(stream, session, wire, &mut report).await;
        }
        Tail::CorruptFault => {
            corrupt_fault_tail(stream, link, session, &mut report).await;
        }
        Tail::PartialFault => {
            partial_fault_tail(stream, link, session, &mut report).await;
        }
        Tail::CtrlCSeam(seam) => {
            ctrl_c_seam_tail(stream, session, seam, &mut report).await;
        }
    }
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

/// The byte-exact image every client in-world frame must carry: opcode 100,
/// header-only, `[usSize=5][ucChk=0][usCmd=100]` (`heartbeat/tests.rs:
/// 71-74`). One helper, every tail — the assertion text is the 6.2 one.
fn assert_heartbeat(plain: &[u8]) {
    assert_eq!(
        u16::from_le_bytes([plain[3], plain[4]]),
        OP_HEARTBEAT,
        "in-world client frames must be heartbeats (opcode 100); \
         got opcode {}",
        u16::from_le_bytes([plain[3], plain[4]])
    );
    assert_eq!(
        plain, &HEARTBEAT_WIRE,
        "the heartbeat must be the header-only image \
         [usSize=5][ucChk=0][usCmd=100] (heartbeat/tests.rs:74)"
    );
}

/// The storm tail (WO-6.3 §6(a)): pace [`STORM_PACE`] ticks of dispatcher
/// frames — the NPC-led opcode-6 batch plus every other opcode the
/// dispatcher owns, and one foreign opcode — until the frame target and
/// [`STORM_MIN_HEARTBEATS`] are both met; then FIN and drain to the
/// client's close.
///
/// The stream splits so one task reads client heartbeats while the pacing
/// loop writes: the two directions never block each other and the ciphers
/// stay in lockstep because each half owns exactly one of them — the link
/// cipher ticks once per encoded frame, the session cipher once per
/// decoded frame (`cipher.rs:23-30`).
async fn storm_tail(
    stream: tokio::net::TcpStream,
    mut link: Cipher,
    mut session: Cipher,
    target_frames: usize,
    report: &mut GameReport,
) {
    let (mut read_half, mut write_half) = stream.into_split();
    let heartbeats = Arc::new(AtomicUsize::new(0));
    let times = Arc::new(Mutex::new(Vec::<Instant>::new()));

    let reader = tokio::spawn({
        let heartbeats = Arc::clone(&heartbeats);
        let times = Arc::clone(&times);
        async move {
            loop {
                match read_ciphered(&mut read_half, &mut session).await {
                    ReadOutcome::Frame { plain, .. } => {
                        assert_heartbeat(&plain);
                        times
                            .lock()
                            .expect("heartbeat times lock")
                            .push(Instant::now());
                        heartbeats.fetch_add(1, Ordering::Relaxed);
                    }
                    // The client's own close after our FIN — the drain
                    // phase every teardown runs (`teardown.rs:366`).
                    ReadOutcome::Eof => break,
                    ReadOutcome::Corrupt => panic!(
                        "a client heartbeat must decode under the session \
                         cipher the handshake seeded"
                    ),
                }
            }
        }
    });

    // The mix (WO-6.3 §6(a)): opcode 6 NPC-led (34 x 68 then the two PCs —
    // LIVE-01's broadcast shape, the regression a PC-only storm would
    // miss), 215, 200, 11, 118, then one foreign opcode the dispatcher
    // routes to `Unhandled` — not an error (`dispatcher.rs:888-918`).
    // Repeat-safety of every in-frame is pinned in the dispatcher suite:
    // 6 upserts, 11 on an absent id is a no-op, 118/200/215 are
    // valid-target-gated, so a legal storm can never end in
    // `Refused{InWorld}`.
    let cycle: [(u16, Vec<u8>); 6] = [
        (OP_LOGINMANY, loginmany_npc_led_body()),
        (OP_PERSON_IP, person_ip_body(NEARBY_UNIQUE as i16).to_vec()),
        (OP_CHARUPDATE, char_update_body(CHAR_UNIQUE as i16).to_vec()),
        (OP_LOGOUT, logout_payload(NEARBY_UNIQUE).to_vec()),
        (
            OP_CHAT,
            chat_body(NEARBY_UNIQUE as i16, "someone", "hi all"),
        ),
        (OP_UNHANDLED_IN_WORLD, vec![0xEE; 8]),
    ];
    let per_tick = target_frames.div_ceil(STORM_TICKS);
    let mut tick = interval(STORM_PACE);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut sent = 0usize;
    let mut cycle_at = 0usize;
    loop {
        tick.tick().await;
        for _ in 0..per_tick {
            let (cmd, payload) = &cycle[cycle_at % cycle.len()];
            let plain = frame(*cmd, payload);
            let wire = encode_link(&mut link, &plain);
            write_half.write_all(&wire).await.expect("storm write");
            sent += 1;
            cycle_at += 1;
        }
        if sent >= target_frames && heartbeats.load(Ordering::Relaxed) >= STORM_MIN_HEARTBEATS {
            break;
        }
    }
    let fin_instant = Instant::now();
    write_half.shutdown().await.expect("storm FIN");
    reader.await.expect("the storm reader task must not panic");

    let observed = heartbeats.load(Ordering::Relaxed);
    report.frames_sent += sent;
    report.frames_received += observed;
    report.heartbeats = observed;
    report.storm_frames = sent;
    report.heartbeat_times = times.lock().expect("heartbeat times lock").clone();
    report.fin_instant = Some(fin_instant);
    // The reader only ever ends on the client's EOF, so reaching here with
    // the join behind us *is* the close observation.
    report.client_closed = true;
}

/// R1's tail: write the malformed wire image verbatim — no cipher, exactly
/// what a broken peer puts on the wire — then drain to the client's close.
/// The client must refuse at `InWorld` with the header guard's own variant
/// and drop the link (the decoder's refusal is sticky, `net/codec.rs`).
async fn raw_fault_tail(
    mut stream: tokio::net::TcpStream,
    mut session: Cipher,
    fault: Vec<u8>,
    report: &mut GameReport,
) {
    stream.write_all(&fault).await.expect("raw fault write");
    drain_to_eof(&mut stream, &mut session, report).await;
}

/// R2's tail: a valid frame with its **last** wire byte flipped — the
/// integrity tag is computed against the last plaintext byte
/// (`cipher.rs:279,295`), so only a tag-side flip trips `Ok(false)`. The
/// client must refuse at `InWorld` with `CodecError::Integrity`.
async fn corrupt_fault_tail(
    mut stream: tokio::net::TcpStream,
    mut link: Cipher,
    mut session: Cipher,
    report: &mut GameReport,
) {
    let mut wire = encode_link(
        &mut link,
        &frame(
            OP_CHAT,
            &chat_body(NEARBY_UNIQUE as i16, "someone", "hi all"),
        ),
    );
    let last = wire.len() - 1;
    wire[last] ^= 0xFF; // the tag-side flip
    stream.write_all(&wire).await.expect("corrupt fault write");
    drain_to_eof(&mut stream, &mut session, report).await;
}

/// R3's tail: three wire bytes of a valid frame (valid `usSize`, no body)
/// then a FIN. The documented behavior this pins: `next_frame` observes
/// EOF, discards the partial and reports the close — **no error**, a clean
/// `Cause::ServerDisconnect` teardown (`game/entry.rs:472-476`).
async fn partial_fault_tail(
    mut stream: tokio::net::TcpStream,
    mut link: Cipher,
    mut session: Cipher,
    report: &mut GameReport,
) {
    let wire = encode_link(
        &mut link,
        &frame(
            OP_CHAT,
            &chat_body(NEARBY_UNIQUE as i16, "someone", "hi all"),
        ),
    );
    stream
        .write_all(&wire[..3])
        .await
        .expect("partial fault write");
    stream.shutdown().await.expect("partial fault FIN");
    drain_to_eof(&mut stream, &mut session, report).await;
}

/// R4's tail: the Ctrl+C seam. Wait out the first byte-exact heartbeat —
/// the in-world loop is then provably live, so the injected watcher's claim
/// lands mid-session instead of racing the loop's start-up check
/// (`driver.rs:666`) — notify the test, then drain to the client's close.
/// The double never FINs here: the client's own teardown closes.
async fn ctrl_c_seam_tail(
    mut stream: tokio::net::TcpStream,
    mut session: Cipher,
    seam: Arc<Notify>,
    report: &mut GameReport,
) {
    match read_ciphered(&mut stream, &mut session).await {
        ReadOutcome::Frame { plain, .. } => {
            assert_heartbeat(&plain);
            report.heartbeats += 1;
            report.frames_received += 1;
        }
        outcome => panic!(
            "the first in-world client frame must be the first heartbeat, \
             got {outcome:?}"
        ),
    }
    seam.notify_one();
    drain_to_eof(&mut stream, &mut session, report).await;
}

/// Reads the client's remaining in-world frames (heartbeats only, asserted
/// byte-exactly) until its close, and records that close as the
/// teardown-close proxy: a client that never tore down never closes, so
/// the EOF is observable evidence the close phase of `terminate` ran
/// (`teardown.rs:366`, WO-6.3 §6(b)).
async fn drain_to_eof(
    stream: &mut tokio::net::TcpStream,
    session: &mut Cipher,
    report: &mut GameReport,
) {
    loop {
        match read_ciphered(stream, session).await {
            ReadOutcome::Frame { plain, .. } => {
                assert_heartbeat(&plain);
                report.heartbeats += 1;
                report.frames_received += 1;
            }
            ReadOutcome::Eof => {
                report.client_closed = true;
                return;
            }
            ReadOutcome::Corrupt => panic!(
                "a client heartbeat must decode under the session cipher the \
                 handshake seeded"
            ),
        }
    }
}
