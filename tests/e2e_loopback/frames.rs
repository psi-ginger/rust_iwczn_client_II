// Step 6.2 — the harness's only byte source (WO-6.2 §4, §5.2).
//
// F-07 at the end-to-end layer: every expectation in this file is hand-placed
// at its literal wire offset with the legacy/server authority cited inline —
// the same dual-sourced fixtures the per-module suites already use, lifted as
// independent byte sources (`CONTRIBUTING_PHASE2.md:70-76`). No client parser
// (`parse_notice`, `WorldList::from_frame`, `parse_login_response`,
// `parse_char_list_entry`, `parse_make_char_info`, `dispatch`, ...) is ever
// called to construct an expected byte, and no expected *ciphertext* is
// derived from the client's own encoder: the cipher instances below are the
// transport transform both real peers perform (the legacy service decodes
// inbound frames with `GetEncrypt()->Decode`, `FrontClientProtocol.cpp:85`;
// the server port keys the same `Cipher` at `main.rs:704`), while the
// expectation itself is always the plaintext literal table.
use std::sync::Mutex;

use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

use goonzu_client::net::cipher::Cipher;

// ---------------------------------------------------------------------------
// Wire header — the 5-byte packed layout every literal below sits inside.
// `usSize` (2, LE) + `ucChk` (1, reserved) + `usCmd` (2, LE)
// (`GoonZu_Source/NetworkLib/ITZNetLib/itzbase.h:63-68`, `#pragma pack(1)`).
// The per-module fixtures write the same layout with the same helper
// (`world_list/tests.rs:89-97`, `login/tests.rs:39-47`,
// `event_loop/tests.rs:99-107`) — this is that helper, restated here so the
// harness shares no code with the modules under test.
// ---------------------------------------------------------------------------

/// Builds `[usSize LE][ucChk=0][usCmd LE][payload]` — one plaintext frame
/// image, pre-cipher.
pub(crate) fn frame(cmd: u16, payload: &[u8]) -> Vec<u8> {
    let size = u16::try_from(5 + payload.len()).expect("frame fits u16");
    let mut bytes = Vec::with_capacity(5 + payload.len());
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes.push(0x00); // ucChk — reserved; the cipher overwrites it on the wire
    bytes.extend_from_slice(&cmd.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

/// The server→client encoding for the game link: one persistent
/// `Cipher::link_cipher()` (fixed keys `0x12/0x34`, `cipher.rs:61-64`)
/// held across every frame the double writes, because the keys *are* the
/// counter and tick once per call (`cipher.rs:23-30`) — the client's inbound
/// decoder installs the same fixed cipher at construct (`entry.rs:399`) and
/// ticks in lockstep, one tick per frame pulled.
pub(crate) fn encode_link(cipher: &mut Cipher, plain: &[u8]) -> Vec<u8> {
    cipher.encode(plain).expect("link encode")
}

/// Reads one ciphered frame the way the services do: `usSize` travels in the
/// clear and selects the length (`cipher.rs:219-220`), then the session
/// cipher decodes the whole frame (`FrontClientProtocol.cpp:85-91`).
pub(crate) async fn read_ciphered(stream: &mut TcpStream, cipher: &mut Cipher) -> ReadOutcome {
    let mut head = [0u8; 2];
    if stream.read_exact(&mut head).await.is_err() {
        return ReadOutcome::Eof;
    }
    let size = usize::from(u16::from_le_bytes(head));
    if size < 5 {
        return ReadOutcome::Corrupt;
    }
    let mut wire = vec![0u8; size];
    wire[..2].copy_from_slice(&head);
    if stream.read_exact(&mut wire[2..]).await.is_err() {
        return ReadOutcome::Corrupt;
    }
    let mut plain = vec![0u8; size];
    if !cipher.decode(&wire, &mut plain).unwrap_or(false) {
        // The integrity check failed — the service would drop the connection
        // here (`FrontClientProtocol.cpp:88-91`); the harness records it.
        return ReadOutcome::Corrupt;
    }
    ReadOutcome::Frame { wire, plain }
}

/// What one read off a double's socket produced.
#[derive(Debug)]
pub(crate) enum ReadOutcome {
    /// A complete frame: its wire bytes and the decoded plaintext image.
    Frame {
        /// Exactly the bytes that travelled.
        wire: Vec<u8>,
        /// The decoded plaintext frame (header included).
        plain: Vec<u8>,
    },
    /// Clean EOF — the peer closed between frames.
    Eof,
    /// A short frame or a failed integrity check: the conversation is broken.
    Corrupt,
}

// ---------------------------------------------------------------------------
// Transcript — every frame both doubles touch, for the root's byte-exact
// conversation assertion (WO-6.2 §6, acceptance item 2).
// ---------------------------------------------------------------------------

/// Which service the frame belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Leg {
    /// The front service (port 5700 leg).
    Front,
    /// The game service (port 4010 leg).
    Game,
}

/// Which way the frame travelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dir {
    /// Server → client: the bytes the double wrote.
    S2C,
    /// Client → server: the bytes the double read (asserted plaintext).
    C2S,
}

/// One transcript entry: the plaintext frame (the expectation surface) plus
/// the raw wire length it travelled as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    /// The service leg.
    pub(crate) leg: Leg,
    /// The direction.
    pub(crate) dir: Dir,
    /// The plaintext frame image — header included, exactly as asserted.
    pub(crate) plain: Vec<u8>,
    /// The length the frame travelled as (equal to `plain.len()`: the cipher
    /// preserves length; plaintext legs are verbatim).
    pub(crate) wire_len: usize,
}

/// Both doubles append here; the root asserts the whole conversation.
#[derive(Debug, Default)]
pub(crate) struct Transcript {
    /// The entries in observation order.
    entries: Mutex<Vec<Entry>>,
}

impl Transcript {
    /// A new, empty transcript.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Records one frame.
    pub(crate) fn push(&self, leg: Leg, dir: Dir, plain: Vec<u8>, wire_len: usize) {
        self.entries.lock().expect("transcript lock").push(Entry {
            leg,
            dir,
            plain,
            wire_len,
        });
    }

    /// A snapshot of everything recorded so far.
    pub(crate) fn snapshot(&self) -> Vec<Entry> {
        self.entries.lock().expect("transcript lock").clone()
    }

    /// The number of entries recorded so far.
    pub(crate) fn len(&self) -> usize {
        self.entries.lock().expect("transcript lock").len()
    }

    /// Whether nothing has been recorded (clippy's `len` companion).
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A deliberate single-byte corruption of one server→client frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mutation {
    /// No corruption — the full-flow path.
    None,
    /// Front stage: `usCount` of the world-list response declared as 2 while
    /// the frame carries one entry — `WorldList::from_frame` must refuse
    /// (`world_list.rs:194-200`, Truncated).
    WorldListCount,
    /// Game-login stage: the occupied roster entry's `szName[0]` becomes a
    /// NUL, so slot 2 reads empty and the driver must refuse the roster
    /// (`driver.rs:598-606`).
    RosterName,
    /// World-entry stage: `uiMakeCharUnique` of the opcode-5 record no
    /// longer matches the handshake's `char_unique` — `apply_login` returns
    /// false and the driver refuses (`world_entry.rs:748-752`,
    /// `driver.rs:634-643`).
    MakeCharUnique,
    /// In-world stage: the chat body's declared `siDataSize` is off by one
    /// from the bytes present — the dispatcher's strict size gate refuses
    /// (`dispatcher.rs:52-67`, `dispatcher/tests.rs:216-241`).
    ChatDataSize,
}

// ---------------------------------------------------------------------------
// Front leg constants — opcode and size authorities. The opcodes are
// hand-placed here (not imported from the client's own tables) so the
// expectation is independent of the module under test
// (`CONTRIBUTING_PHASE2.md:74-76`).
// ---------------------------------------------------------------------------

/// `FRONTMSG_RESPONSE_SECRETKEY = 30000` (`FrontMsg.h:114`,
/// `front.rs:93-94`).
pub(crate) const OP_SECRETKEY: u16 = 30000;
/// `FRONTMSG_REQUEST_NOTICE = 2` (`FrontMsg.h:45`, `world_list.rs:134-135`).
pub(crate) const OP_REQUEST_NOTICE: u16 = 2;
/// `FRONTMSG_RESPONSE_NOTICE = 1001` (`FrontMsg.h:94`).
pub(crate) const OP_RESPONSE_NOTICE: u16 = 1001;
/// `FRONTMSG_REQUEST_WORLDLIST = 3` (`FrontMsg.h:46`).
pub(crate) const OP_REQUEST_WORLDLIST: u16 = 3;
/// `FRONTMSG_RESPONSE_WORLDLIST = 1002` (`FrontMsg.h:95`).
pub(crate) const OP_RESPONSE_WORLDLIST: u16 = 1002;
/// `FRONTMSG_REQUEST_LOGIN = 1` (`FrontMsg.h:336-348`, `login.rs:155`).
pub(crate) const OP_REQUEST_LOGIN: u16 = 1;
/// `FRONTMSG_RESPONSE_LOGIN = 1000` (`FrontMsg.h:419-442`, `login.rs:160`).
pub(crate) const OP_RESPONSE_LOGIN: u16 = 1000;

/// The front handshake frame as it sits on the wire, in the clear (the front
/// link's inbound half is plaintext — `front.rs` module docs, asymmetry
/// table): `usSize` 10, `ucChk` 0, `usCmd` 30000 LE, then the seed slots
/// `0xA7`/`0x5C` at payload offsets 2 and 4 (`sResponse_SecretKey`,
/// `FrontMsg.h:132-141`). Lifted verbatim from the Step 3.1 fixture
/// (`world_list/tests.rs:22`).
pub(crate) const SECRETKEY_WIRE: [u8; 10] =
    [0x0A, 0x00, 0x00, 0x30, 0x75, 0x00, 0x00, 0xA7, 0x00, 0x5C];
/// `ucKey1` (`world_list/tests.rs:23`) — the first session-cipher seed the
/// double hands out; the client keys its outbound encoder with it
/// (`front.rs:257-277`), so the double decodes with the same value.
pub(crate) const FRONT_KEY1: u8 = 0xA7;
/// `ucKey2` (`world_list/tests.rs:24`).
pub(crate) const FRONT_KEY2: u8 = 0x5C;

/// The notice response's payload: exactly `NOTICE_PAYLOAD_LEN` = 16 bytes of
/// C-string text (`FrontClientProtocol.cpp:760`; the server port's
/// `NOTICE_PAYLOAD_LEN`, `gz-proto/src/front.rs:26-28`;
/// `world_list.rs:162-166`).
pub(crate) const NOTICE_TEXT: [u8; 16] = *b"WELCOME\0\0\0\0\0\0\0\0\0";

/// `sizeof(WORLDINFO)` = 428 under the MSVC default packing of
/// `FrontMsg.h:6-41` (`world_list.rs:157-160`; the server port's
/// `WORLDINFO_LEN`, `gz-proto/src/front.rs:31`).
pub(crate) const WORLDINFO_LEN: usize = 428;

/// One `WORLDINFO` entry (428 bytes) with every field set at its literal
/// wire offset — the layout and values mirror the Step 3.2 fixture
/// (`world_list/tests.rs:47-77`), whose offsets come from the packed
/// declaration (`FrontMsg.h:6-41`, pads corroborated by
/// `gz-proto/src/front.rs:210-233`), never from the parser under test.
pub(crate) fn worldinfo_entry() -> [u8; WORLDINFO_LEN] {
    let mut e = [0u8; WORLDINFO_LEN];
    e[0..2].copy_from_slice(&7u16.to_le_bytes()); // baseinfo.worldId
    e[2..9].copy_from_slice(b"World-A"); // worldname[20], NUL-terminated
    e[22..24].fill(0xFF); // pad (align siServerID)
    e[24..28].copy_from_slice(&0x1122_3344i32.to_le_bytes()); // siServerID
    e[28..36].copy_from_slice(b"10.1.2.3"); // gameserverip[16], NUL-padded
    e[44..46].copy_from_slice(&4010u16.to_le_bytes()); // gameserverport
    e[46..56].copy_from_slice(b"2004-07-13"); // opendate[15]
    e[61..74].copy_from_slice(b"world message"); // shortmessage[64]
    e[125..128].fill(0xFF); // pad (struct tail)
    e[128..130].copy_from_slice(&7u16.to_le_bytes()); // refresh.worldId
    e[130..132].copy_from_slice(&0u16.to_le_bytes()); // refresh.state = normal
    e[132..134].copy_from_slice(&42u16.to_le_bytes()); // currentConnections
    e[136..142].copy_from_slice(b"GoonZu"); // goonzuname
    e[156..169].copy_from_slice(b"server banner"); // goonzutext
    e[412..416].copy_from_slice(&100i32.to_le_bytes()); // population
    e
}

/// `szAccountID[20]` (`FrontMsg.h:490`): the world-list request's payload,
/// `ACCOUNT_ID_LEN` bytes, NUL-padded (`world_list.rs:152-155`).
pub(crate) fn account_id_payload(account: &str) -> [u8; 20] {
    let mut payload = [0u8; 20];
    let raw = account.as_bytes();
    assert!(
        raw.len() < 20,
        "the test account must fit a NUL-terminated szAccountID[20]"
    );
    payload[..raw.len()].copy_from_slice(raw);
    payload
}

/// `sizeof(sRequest_LogIn)` body: `userid`/`userpw`/`macaddress` at 0/20/40,
/// each `szCredentialLen` = 20 bytes NUL-padded (`FrontMsg.h:336-348`,
/// `login.rs:155-171`).
pub(crate) fn login_request_payload(user: &str, password: &str, mac: &str) -> [u8; 60] {
    let mut payload = [0u8; 60];
    for (offset, raw) in [
        (0usize, user.as_bytes()),
        (20, password.as_bytes()),
        (40, mac.as_bytes()),
    ] {
        assert!(
            raw.len() < 20,
            "test credentials must fit a NUL-terminated 20-byte slot"
        );
        payload[offset..offset + raw.len()].copy_from_slice(raw);
    }
    payload
}

/// `sizeof(sResponse_LogIn)` = 108 (`FrontMsg.h:419-442`; the server port's
/// `LOGIN_RESPONSE_PAYLOAD_LEN`, `gz-proto/src/front.rs:24`;
/// `login.rs:176`). The response this double answers with: `usResult` 0,
/// `siAuthKey` = [`AUTH_KEY`], `userid` = [`ECHO_USER_ID`] (the canonical id
/// the legacy copies back at login, `Client-Winsock.cpp:377` — the id the
/// game login must then carry, Step 4.3), `siAccountStatus` 1 (what the
/// server port sends on success, `main.rs:255-262`; `-1` is the rejection
/// sentinel, `login.rs:450-452`). Offsets per `login.rs:496-513`.
pub(crate) fn login_response_payload() -> [u8; 108] {
    let mut p = [0u8; 108];
    p[0..2].copy_from_slice(&0u16.to_le_bytes()); // usResult = 0 (accepted)
    p[2..6].copy_from_slice(&AUTH_KEY.to_le_bytes()); // siAuthKey
    p[6..6 + ECHO_USER_ID.len()].copy_from_slice(ECHO_USER_ID); // userid[20]
    p[103..107].copy_from_slice(&1i32.to_le_bytes()); // siAccountStatus = 1
    p
}

/// The account's `idnum`, handed out as the front login's `siAuthKey`
/// (`FrontMsg.h:424`; `gz-proto/src/front.rs:97`) and carried into the game
/// login (`login.rs:310-318`). One value, both legs: the game double
/// asserts the client delivered it back in the opcode-2 payload.
pub(crate) const AUTH_KEY: i32 = 0x0BADC0DE;

/// The canonical account id the login response echoes in `userid[20]`
/// (`FrontMsg.h:427`) — the id the game login's `szID` must then carry
/// (`login.rs:228-229`, Step 4.3's MOD-04 handoff), NOT the id the test
/// typed into the front requests.
pub(crate) const ECHO_USER_ID: &[u8] = b"haru";

// ---------------------------------------------------------------------------
// Game leg constants. Opcode authorities: `MsgType.h` via the port's
// `opcodes_game.rs`; payload lengths from the binary-measured / sizeof
// authorities cited per constant.
// ---------------------------------------------------------------------------

/// `GAMEMSG_RESPONSE_CLIENTCHARUNIQUE = 1` — the handshake opcode
/// (`MsgType.h:22`, `entry.rs:175-178`).
pub(crate) const OP_CLIENTCHARUNIQUE: u16 = 1;
/// `GAMEMSG_REQUEST_LOGIN = 2` (`MsgType.h:2963-2967`, `game/login.rs:129`).
pub(crate) const OP_GAME_LOGIN: u16 = 2;
/// `GAMEMSG_RESPONSE_CHARLIST = 3` (`MsgType.h`, `char_info.rs`).
pub(crate) const OP_CHARLIST: u16 = 3;
/// `GAMEMSG_REQUEST_SELECTCHAR = 4` (`MsgType.h:26`, `world_entry.rs:120-123`).
pub(crate) const OP_SELECTCHAR: u16 = 4;
/// `GAMEMSG_RESPONSE_LOGIN = 5` — the world-entry `clMakeCharInfo` record
/// (`char_info.rs:10-12`).
pub(crate) const OP_MAKE_CHAR: u16 = 5;
/// `GAMEMSG_RESPONSE_LOGINMANY = 6` (`MsgType.h:28`).
pub(crate) const OP_LOGINMANY: u16 = 6;
/// `GAMEMSG_RESPONSE_LOGOUT = 11` (`MsgType.h:36`).
pub(crate) const OP_LOGOUT: u16 = 11;
/// `GAMEMSG_RESPONSE_SERVERINFO = 106` (`MsgType.h`, `world_entry.rs:125-128`).
pub(crate) const OP_SERVERINFO: u16 = 106;
/// `GAMEMSG_REQUEST_HEARTBEAT = 100` (`MsgType.h:56`,
/// `heartbeat/tests.rs:76-82`).
pub(crate) const OP_HEARTBEAT: u16 = 100;
/// `GAMEMSG_RESPONSE_CHAT = 118` (`MsgType.h:87`).
pub(crate) const OP_CHAT: u16 = 118;
/// `GAMEMSG_RESPONSE_CHARUPDATE = 215`-neighbour: `GAMEMSG_RESPONSE_CHARUPDATE
/// = 200` (`MsgType.h:220`).
pub(crate) const OP_CHARUPDATE: u16 = 200;
/// `GAMEMSG_RESPONSE_PERSON_IP = 215` (`MsgType.h:252`).
pub(crate) const OP_PERSON_IP: u16 = 215;
/// `GAMEMSG_REQUEST_VILLAGESTRUCT = 1101` (`MsgType.h:590`,
/// `world_entry.rs:130-133`).
pub(crate) const OP_VILLAGESTRUCT_REQ: u16 = 1101;
/// `GAMEMSG_RESPONSE_VILLAGESTRUCT = 1102` (`MsgType.h:590`).
pub(crate) const OP_VILLAGESTRUCT_RES: u16 = 1102;

/// The session `char_unique` the double hands out in the opcode-1
/// handshake. Fits `u16` because `cltMakeCharInfo.uiMakeCharUnique` is
/// 16 bits (`char_info.rs:501-504`) and `apply_login` compares
/// `i32::from(u16)` against the handshake's `i32`
/// (`world_entry.rs:748-752`).
pub(crate) const CHAR_UNIQUE: i32 = 4242;
/// The session-cipher keys the double hands out in the handshake — the
/// values the Step 5.2 fixture negotiates (`heartbeat/tests.rs:56-57`).
/// The server decodes the client's outbound frames with
/// `Cipher::session_cipher(key1, key2)` (`main.rs:704`), so the double
/// decodes with the same.
pub(crate) const GAME_KEY1: u8 = 0x7E;
pub(crate) const GAME_KEY2: u8 = 0xAD;
/// The fixed link-cipher keys the *server→client* half travels under: the
/// client's inbound decoder is keyed `0x12/0x34` at construct
/// (`entry.rs:396-400`; `cipher.rs:61-64`), so the double encodes with the
/// same fixed cipher.
pub(crate) const LINK_KEY1: u8 = 0x12;
/// See [`LINK_KEY1`].
pub(crate) const LINK_KEY2: u8 = 0x34;

/// `MAX_TOTAL_CHAR_PER_USER` = 3 + 12 = 15 (`CommonHeader.h:579-581`,
/// `char_info.rs:83-87`): the roster is exactly this many opcode-3 frames.
pub(crate) const ROSTER_SLOTS: usize = 15;
/// The roster slot this session occupies: `uiIndex` 2 with a non-empty
/// `szName`. Chosen in `0..SELECT_CHAR_SLOT_MAX` (= 3,
/// `world_entry.rs:189`) because `request_select_char` refuses anything at
/// or above it (`world_entry.rs:286-291`) — the driver's own config
/// validation only caps at `CHAR_SLOTS`.
pub(crate) const OCCUPIED_SLOT: i32 = 2;
/// `sizeof(cltCharListInfo)` = 904 + 64 + 16 = 984
/// (`Char-Common.h:453-471`, `char_info.rs:89-95`); `usSize` 989.
pub(crate) const CHAR_LIST_PAYLOAD_LEN: usize = 984;
/// `sizeof(cltMakeCharInfo)` = 592 (`CommonHeader.h:2166`; binary-measured,
/// `char_info.rs:31,119`); `usSize` 597.
pub(crate) const MAKE_CHAR_PAYLOAD_LEN: usize = 592;
/// `sizeof(cltMakeCharInfoCommon)` = 68 — the opcode-6 NPC stride,
/// dual-sourced: the MSVC field sum (`CommonHeader.h:1967-1995`: 16 bitfield
/// words + 4 SI08/SI16 pair + 20 `clOrder` + 12 `clIdentity` + 16
/// `clInnerStatus`) and the live wire (`evidence/live/decode2.log`,
/// `payload=2312` = 34 x 68, every head `uiType = 0`).
pub(crate) const MAKE_CHAR_COMMON_PAYLOAD_LEN: usize = 68;
/// `sizeof(cltGameMsgResponse_ServerInfo)` = 48 (binary-measured,
/// `game.rs:35`, `world_entry.rs:154-158`); `usSize` 53.
pub(crate) const SERVER_INFO_PAYLOAD_LEN: usize = 48;
/// `sizeof(cltGameMsgResponse_VillageStruct)` = 2 + 10*2 = 22
/// (`MsgType.h:3085-3102`, `world_entry.rs:160-165`); `usSize` 27.
pub(crate) const VILLAGE_STRUCT_PAYLOAD_LEN: usize = 22;
/// The service-area banner this build answers with: `SERVICE_AREA_ENGLISH`
/// = 0x08 (`world_entry.rs:84-85,197`; the server port advertises it).
pub(crate) const SERVICE_AREA_ENGLISH: i16 = 0x08;
/// The fixed 20-byte prefix of a chat body before the name/text region
/// (`MsgType-System.h:1139`, `dispatcher.rs:169`).
pub(crate) const CHAT_FIXED_LEN: usize = 20;
/// `sizeof(cltBasicCharUpdateInfo)` = 24 at `/Zp8`
/// (`CommonLogic.h:383-410`, `dispatcher.rs:182`).
pub(crate) const CHAR_UPDATE_PAYLOAD_LEN: usize = 24;
/// `PERSON_IP_LEN` = 8 + 64 = 72 (binary-measured, `game.rs:426`,
/// `dispatcher.rs:186`).
pub(crate) const PERSON_IP_PAYLOAD_LEN: usize = 72;
/// The heartbeat's plaintext image, pre-cipher: `usSize` 5 LE, `ucChk` 0,
/// `usCmd` 100 LE — hand-written from the header layout in the Step 5.2
/// fixture (`heartbeat/tests.rs:71-74`), never produced by any encoder.
pub(crate) const HEARTBEAT_WIRE: [u8; 5] = [0x05, 0x00, 0x00, 0x64, 0x00];
/// `MAKECHARTYPE_PC` = 1 — the record type bit at 0x00 of a MakeCharInfo
/// record (`char_info.rs:495-497`; the Step 4.4/5.4 fixtures place
/// `MAKECHARTYPE_PC | (0x2A << 1)`, `dispatcher/tests.rs:175`).
pub(crate) const MAKECHARTYPE_PC: u32 = 1;
/// `MAKECHARTYPE_NPC` = 0 — the same `uiType` bit cleared
/// (`CommonHeader.h:1892`).
pub(crate) const MAKECHARTYPE_NPC: u32 = 0;

/// The opcode-1 handshake's 8-byte payload: `SI32` char_unique, `UI08`
/// key1, `UI08` key2, `UI16` version (`cltGameMsgResponse_ClientCharUnique`,
/// `entry.rs:38-60`, parsed at `entry.rs:251-256`).
pub(crate) fn handshake_payload() -> [u8; 8] {
    let mut p = [0u8; 8];
    p[0..4].copy_from_slice(&CHAR_UNIQUE.to_le_bytes());
    p[4] = GAME_KEY1;
    p[5] = GAME_KEY2;
    p[6..8].copy_from_slice(&0i16.to_le_bytes()); // version
    p
}

/// The 48-byte `cltLogInInfo` the game login sends (`CommonHeader.h:
/// 1442-1452`): `siAuthKey` at 0..4 (the front's token — the client carries
/// `FrontAuth::auth_key`, `login.rs:240-247`), `szID` at 4..24 (the
/// *response's* canonical id, not the typed one), `szPassword` at 24..44,
/// `uiGameMode` 0 at 44 (`GAME_MODE_WINDOWED`, `game/login.rs:147`),
/// three zero pad bytes (`game/login.rs:196-221`).
pub(crate) fn game_login_payload(password: &str) -> [u8; 48] {
    let mut p = [0u8; 48];
    p[0..4].copy_from_slice(&AUTH_KEY.to_le_bytes());
    p[4..4 + ECHO_USER_ID.len()].copy_from_slice(ECHO_USER_ID);
    let raw = password.as_bytes();
    assert!(raw.len() < 20, "test password must fit szPassword[20]");
    p[24..24 + raw.len()].copy_from_slice(raw);
    p[44] = 0; // uiGameMode = GAME_MODE_WINDOWED
    p
}

/// One 984-byte roster slot: `clBI.siIDNum` at 0..4 (the account's idnum),
/// `clBI.szAccountID[20]` at 4..24, `clBI.uiIndex` at 24 (the slot),
/// `clBI.clPerson.szName[20]` at 80..100 (`Char-Common.h:453-471`;
/// offsets from the binary, `char_info.rs:239-264`). The remaining bytes
/// stay zero — the legacy sends zero-filled struct tails for empty slots
/// and this port parses only the fields above plus the `clIP` block
/// (`char_info.rs:256-264`), all valid as zero.
pub(crate) fn roster_entry(
    slot: u8,
    account: &str,
    name: Option<&str>,
) -> [u8; CHAR_LIST_PAYLOAD_LEN] {
    let mut p = [0u8; CHAR_LIST_PAYLOAD_LEN];
    p[0..4].copy_from_slice(&AUTH_KEY.to_le_bytes()); // siIDNum = idnum
    let raw = account.as_bytes();
    assert!(raw.len() < 20, "test account must fit szAccountID[20]");
    p[4..4 + raw.len()].copy_from_slice(raw);
    p[24] = slot; // uiIndex
    if let Some(name) = name {
        let raw = name.as_bytes();
        assert!(raw.len() < 20, "test character name must fit szName[20]");
        p[80..80 + raw.len()].copy_from_slice(raw);
    }
    p
}

/// `GAMEMSG_REQUEST_SELECTCHAR`'s body: a bare `SI32` slot index
/// (`NSelectChar.cpp:1353`, `world_entry.rs:139-147`).
pub(crate) fn select_char_payload(slot: i32) -> [u8; 4] {
    slot.to_le_bytes()
}

/// `GAMEMSG_REQUEST_VILLAGESTRUCT`'s body: a bare `SI32` village ref
/// (`Client-Login.cpp:281`, `world_entry.rs:147-152`).
pub(crate) fn village_request_payload(village_ref: i32) -> [u8; 4] {
    village_ref.to_le_bytes()
}

/// The 48-byte `cltGameMsgResponse_ServerInfo`: `siServiceArea` at 0..2 —
/// the one field this flow depends on, since the SERVERINFO handler returns
/// without storing anything when the banner names another region
/// (`Client-System.cpp:1682`, `world_entry.rs:464-507`). The double answers
/// with [`SERVICE_AREA_ENGLISH`]; the mutation tests flip exactly this.
pub(crate) fn server_info_payload() -> [u8; SERVER_INFO_PAYLOAD_LEN] {
    let mut p = [0u8; SERVER_INFO_PAYLOAD_LEN];
    p[0..2].copy_from_slice(&SERVICE_AREA_ENGLISH.to_le_bytes());
    p[2..4].copy_from_slice(&1i16.to_le_bytes()); // siServerUnique
    p
}

/// One 592-byte `MAKECHARTYPE_PC` record from the Step 4.4 field table
/// (`CommonHeader.h:1969-1977`, low bits first; the tail fields the port
/// reads sit at binary-measured offsets, `char_info.rs` `decode_record`) —
/// the same arithmetic the Step 5.4 fixture uses (`dispatcher/tests.rs
/// pc_record`).
pub(crate) fn make_char_record(unique: u16, level: u8) -> [u8; MAKE_CHAR_PAYLOAD_LEN] {
    let mut record = [0u8; MAKE_CHAR_PAYLOAD_LEN];
    let type_kind = MAKECHARTYPE_PC | (0x2A_u32 << 1); // uiType | uiMakeCharKind
    let unique_level = u32::from(unique) | (u32::from(level) << 16);
    let life_x = 777u32 | (11u32 << 20); // uiLife | uiX
    let y_map_speed = 22u32 | (3u32 << 10) | (4u32 << 20); // uiY|uiMapIndex|speed
    record[0..4].copy_from_slice(&type_kind.to_le_bytes());
    record[4..8].copy_from_slice(&unique_level.to_le_bytes());
    record[8..12].copy_from_slice(&life_x.to_le_bytes());
    record[12..16].copy_from_slice(&y_map_speed.to_le_bytes());
    record
}

/// One 68-byte `MAKECHARTYPE_NPC` record: the `cltMakeCharInfoCommon` head
/// from the same field table with `uiType` cleared and the PC-only tail
/// absent — the record form the live server's world-entry broadcast is made
/// of (`evidence/live/decode2.log`: rec[00] carries kind 0x23, level 1,
/// life 40, x 20, y 338, map 158, speed 2 and byte 56 = 0x01 =
/// `ENEMYTYPE_ALLPC` at `clEnemyType`; stride dual-sourced at
/// [`MAKE_CHAR_COMMON_PAYLOAD_LEN`]).
pub(crate) fn make_npc_record(unique: u16) -> [u8; MAKE_CHAR_COMMON_PAYLOAD_LEN] {
    let mut record = [0u8; MAKE_CHAR_COMMON_PAYLOAD_LEN];
    let type_kind = MAKECHARTYPE_NPC | (0x23_u32 << 1); // uiType | uiMakeCharKind
    let unique_level = u32::from(unique) | (1u32 << 16); // uiLevel = 1
    let life_x = 40u32 | (20u32 << 20); // uiLife | uiX
    let y_map_speed = 338u32 | (158u32 << 10) | (2u32 << 20); // y|map|speed
    record[0..4].copy_from_slice(&type_kind.to_le_bytes());
    record[4..8].copy_from_slice(&unique_level.to_le_bytes());
    record[8..12].copy_from_slice(&life_x.to_le_bytes());
    record[12..16].copy_from_slice(&y_map_speed.to_le_bytes());
    record[0x38..0x3A].copy_from_slice(&0x0001u16.to_le_bytes()); // clEnemyType
    record
}

/// The mixed opcode-6 body the in-world set leads with: one NPC record (the
/// live broadcast's NPC-led shape, LIVE-01), then the local and the nearby
/// player records — the legacy walk strides each by its own `uiType`
/// (`Client-Login.cpp:62-75`). Shared by the expected conversation and the
/// game double so the transcript comparison stays byte-exact by
/// construction; the per-offset assertions live in the unit suites.
pub(crate) fn loginmany_body() -> Vec<u8> {
    let mut body = Vec::with_capacity(MAKE_CHAR_COMMON_PAYLOAD_LEN + 2 * MAKE_CHAR_PAYLOAD_LEN);
    body.extend_from_slice(&make_npc_record(8500)); // decode2.log's first id
    body.extend_from_slice(&make_char_record(CHAR_UNIQUE as u16, 12));
    body.extend_from_slice(&make_char_record(NEARBY_UNIQUE as u16, 9));
    body
}

/// The 22-byte `cltGameMsgResponse_VillageStruct`: `SI16 VillageUnique` at
/// 0..2 (non-negative — a negative answer is refused,
/// `world_entry.rs:274-280`) + `SI16 siVillageStructureInfo[10]`
/// (`MsgType.h:3085-3102`, `world_entry.rs:552-566`).
pub(crate) fn village_struct_payload(village_ref: i32) -> [u8; VILLAGE_STRUCT_PAYLOAD_LEN] {
    let mut p = [0u8; VILLAGE_STRUCT_PAYLOAD_LEN];
    p[0..2].copy_from_slice(&(village_ref as i16).to_le_bytes());
    p
}

/// `GAMEMSG_RESPONSE_LOGOUT`'s body: exactly one `SI32` (the two-argument
/// `cltMsg` constructor, `MsgType.h:2977-2983`, `dispatcher.rs:44-50`).
pub(crate) fn logout_payload(char_unique: i32) -> [u8; 4] {
    char_unique.to_le_bytes()
}

/// A chat body the legacy constructor's arithmetic produces: 20 fixed bytes,
/// then name + text + the NUL `_tcslen` counts (`MsgType-System.h:1139-1158`,
/// `dispatcher.rs:52-67`); the declared `siDataSize` is computed from the
/// placed bytes exactly as the Step 5.4 fixture does
/// (`dispatcher/tests.rs:54-77`).
pub(crate) fn chat_body(char_unique: i16, player_name: &str, text: &str) -> Vec<u8> {
    let mut region = player_name.as_bytes().to_vec();
    region.extend_from_slice(text.as_bytes());
    region.push(0); // the NUL `_tcslen` counts
    let declared = (CHAT_FIXED_LEN + region.len()) as i16;
    let mut body = vec![0u8; CHAT_FIXED_LEN + region.len()];
    body[0..2].copy_from_slice(&declared.to_le_bytes()); // siDataSize
    body[2..4].copy_from_slice(&char_unique.to_le_bytes()); // siCharUnique
    body[4..6].copy_from_slice(&(player_name.len() as i16).to_le_bytes());
    body[6..8].copy_from_slice(&1i16.to_le_bytes()); // siChatMode
    body[8] = 0; // bGameMaster = false
    body[12..16].copy_from_slice(&0i32.to_le_bytes()); // siSystemTxt
    body[16..20].copy_from_slice(&9i32.to_le_bytes()); // siLevel
    body[CHAT_FIXED_LEN..].copy_from_slice(&region);
    body
}

/// A 24-byte `cltBasicCharUpdateInfo` from the field table
/// (`CommonLogic.h:385-394`, `/Zp8`: the SI08 and bool pack at 2..4, the
/// tail byte is alignment pad) — the Step 5.4 fixture's placements
/// (`dispatcher/tests.rs:83-110`).
pub(crate) fn char_update_body(char_unique: i16) -> [u8; CHAR_UPDATE_PAYLOAD_LEN] {
    let mut body = [0u8; CHAR_UPDATE_PAYLOAD_LEN];
    body[0..2].copy_from_slice(&char_unique.to_le_bytes());
    body[2] = 3; // siMoveSpeed
    body[3] = 1; // bUrgentMode = true
    body[4..8].copy_from_slice(&4321i32.to_le_bytes()); // siLife
    body[8..12].copy_from_slice(&90i32.to_le_bytes()); // siReservedLife
    body[12..16].copy_from_slice(&88i32.to_le_bytes()); // siMana
    body[16..18].copy_from_slice(&300i16.to_le_bytes()); // siX
    body[18..20].copy_from_slice(&400i16.to_le_bytes()); // siY
    body[20] = 1; // siGMMode
    body[21] = 1; // bPCRoom = true
    body[22] = 6; // siMakeAniType
    body[23] = 0xEE; // alignment pad — parsers must ignore it
    body
}

/// A 72-byte `cltGameMsgResponse_IP`: `siCharUnique` at 0, the 64-byte
/// `cltPIPInfo` block at 8, the `/Zp8` pad between filled with a sentinel
/// the parser must ignore (`game.rs:426`, `dispatcher.rs:135-160`) — the
/// Step 5.4 fixture's placements over its `wire_pip` values
/// (`dispatcher/tests.rs:114-160`).
pub(crate) fn person_ip_body(char_unique: i16) -> [u8; PERSON_IP_PAYLOAD_LEN] {
    let mut body = [0u8; PERSON_IP_PAYLOAD_LEN];
    body[0..2].copy_from_slice(&char_unique.to_le_bytes());
    body[2..8].fill(0xEE); // /Zp8 pad
    let b = 8;
    for (i, value) in [11i32, 12, 13, 14, 15, 16, 17].iter().enumerate() {
        let off = b + i * 4;
        body[off..off + 4].copy_from_slice(&value.to_le_bytes()); // str..wis
    }
    body[b + 28..b + 30].copy_from_slice(&34i16.to_le_bytes()); // siLevel
    body[b + 30..b + 32].copy_from_slice(&5i16.to_le_bytes()); // siBonus
    body[b + 32..b + 36].copy_from_slice(&161i32.to_le_bytes()); // Life
    body[b + 36..b + 40].copy_from_slice(&40i32.to_le_bytes()); // Mana
    body[b + 40..b + 48].copy_from_slice(&0x0102_0304_0506_0708i64.to_le_bytes()); // siExp
    body[b + 48..b + 50].copy_from_slice(&1i16.to_le_bytes()); // siFameLevel
    body[b + 50..b + 52].copy_from_slice(&2i16.to_le_bytes()); // siSkillBonus
    body[b + 52..b + 56].copy_from_slice(&3i32.to_le_bytes()); // siFameExp
    body[b + 56..b + 64].copy_from_slice(&999_999i64.to_le_bytes()); // siMoney
    body
}

// ---------------------------------------------------------------------------
// The expected conversation (WO-6.2 §6): every frame of the happy flow, in
// order, both directions. The full-flow test asserts the transcript equals
// this table entry-for-entry — one assertion that fails loudly on any
// added, dropped, reordered or altered frame on either leg.
// ---------------------------------------------------------------------------

/// Builds one transcript [`Entry`].
fn entry(leg: Leg, dir: Dir, plain: Vec<u8>) -> Entry {
    let wire_len = plain.len(); // the cipher preserves length; plaintext legs are verbatim
    Entry {
        leg,
        dir,
        plain,
        wire_len,
    }
}

/// The happy-path conversation up to (and including) the last server frame
/// of the world-entry leg. The in-world tail is heartbeats only: the root
/// appends one [`Entry`] per observed heartbeat (at least one is required
/// by the full-flow test).
pub(crate) fn expected_conversation(password: &str, account: &str, mac: &str) -> Vec<Entry> {
    // Front leg: plaintext server frames, session-ciphered client frames.
    let mut expected = vec![
        entry(Leg::Front, Dir::S2C, SECRETKEY_WIRE.to_vec()),
        entry(Leg::Front, Dir::C2S, frame(OP_REQUEST_NOTICE, &[])),
        entry(
            Leg::Front,
            Dir::S2C,
            frame(OP_RESPONSE_NOTICE, &NOTICE_TEXT),
        ),
        entry(
            Leg::Front,
            Dir::C2S,
            frame(OP_REQUEST_WORLDLIST, &account_id_payload(account)),
        ),
        {
            let mut payload = vec![0x01, 0x00]; // usCount = 1 (LE)
            payload.extend_from_slice(&worldinfo_entry());
            entry(Leg::Front, Dir::S2C, frame(OP_RESPONSE_WORLDLIST, &payload))
        },
        entry(
            Leg::Front,
            Dir::C2S,
            frame(
                OP_REQUEST_LOGIN,
                &login_request_payload(account, password, mac),
            ),
        ),
        entry(
            Leg::Front,
            Dir::S2C,
            frame(OP_RESPONSE_LOGIN, &login_response_payload()),
        ),
        // Game leg: every server frame is link-ciphered on the wire; the
        // plaintext image is what the transcript records and asserts.
        entry(
            Leg::Game,
            Dir::S2C,
            frame(OP_CLIENTCHARUNIQUE, &handshake_payload()),
        ),
        entry(
            Leg::Game,
            Dir::C2S,
            frame(OP_GAME_LOGIN, &game_login_payload(password)),
        ),
    ];
    for slot in 0..ROSTER_SLOTS {
        let name = if slot as i32 == OCCUPIED_SLOT {
            Some("Hero")
        } else {
            None
        };
        expected.push(entry(
            Leg::Game,
            Dir::S2C,
            frame(OP_CHARLIST, &roster_entry(slot as u8, account, name)),
        ));
    }
    expected.extend([
        entry(
            Leg::Game,
            Dir::C2S,
            frame(OP_SELECTCHAR, &select_char_payload(OCCUPIED_SLOT)),
        ),
        entry(
            Leg::Game,
            Dir::S2C,
            frame(OP_SERVERINFO, &server_info_payload()),
        ),
        entry(
            Leg::Game,
            Dir::S2C,
            frame(OP_MAKE_CHAR, &make_char_record(CHAR_UNIQUE as u16, 12)),
        ),
        entry(
            Leg::Game,
            Dir::C2S,
            frame(OP_VILLAGESTRUCT_REQ, &village_request_payload(VILLAGE_REF)),
        ),
        entry(
            Leg::Game,
            Dir::S2C,
            frame(OP_VILLAGESTRUCT_RES, &village_struct_payload(VILLAGE_REF)),
        ),
        // The in-world set: 6 (a mixed NPC + PC batch — one NPC record, the
        // live broadcast's shape, then two PC records: the local character
        // and a nearby one), 11 (the nearby one logs out), 118 (chat), 200
        // (state sync), 215 (HP/MP) — `dispatcher.rs:16-28`.
        entry(Leg::Game, Dir::S2C, frame(OP_LOGINMANY, &loginmany_body())),
        entry(
            Leg::Game,
            Dir::S2C,
            frame(OP_LOGOUT, &logout_payload(NEARBY_UNIQUE)),
        ),
        entry(
            Leg::Game,
            Dir::S2C,
            frame(
                OP_CHAT,
                &chat_body(NEARBY_UNIQUE as i16, "someone", "hi all"),
            ),
        ),
        entry(
            Leg::Game,
            Dir::S2C,
            frame(OP_CHARUPDATE, &char_update_body(CHAR_UNIQUE as i16)),
        ),
        entry(
            Leg::Game,
            Dir::S2C,
            frame(OP_PERSON_IP, &person_ip_body(CHAR_UNIQUE as i16)),
        ),
    ]);
    expected
}

/// The nearby character the opcode-6 body introduces and the opcode-11 body
/// removes (`dispatcher.rs:913-930,1011-1027`).
pub(crate) const NEARBY_UNIQUE: i32 = 7777;
/// The village blueprint the driver requests; inside `1..MAX_VILLAGE_NUMBER`
/// (= 100, `world_entry.rs:199-204`).
pub(crate) const VILLAGE_REF: i32 = 7;

/// The corrupted world-list response the [`Mutation::WorldListCount`] path
/// writes: `usCount` declared as 2 while the frame still carries one entry,
/// so `WorldList::from_frame` must refuse with Truncated
/// (`world_list.rs:194-200`).
pub(crate) fn mutated_worldlist_response() -> Vec<u8> {
    let mut payload = vec![0x02, 0x00]; // usCount = 2 (LE) — the flipped byte
    payload.extend_from_slice(&worldinfo_entry());
    frame(OP_RESPONSE_WORLDLIST, &payload)
}

// ---------------------------------------------------------------------------
// The byte-source's own pin-down: the two assumptions the doubles build on
// that live *outside* this file's literals — the SECRETKEY frame's header
// arithmetic and the link cipher's fixed keys — are asserted here so they
// cannot drift silently (they are hand-placed, not derived from any client
// code path).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The SECRETKEY fixture must be exactly what its citation says: `usSize`
    /// 10 LE, the reserved `ucChk` zero, `usCmd` [`OP_SECRETKEY`] (30000)
    /// LE — checked field by field against independent arithmetic, not
    /// against any encoder (`FrontMsg.h:114,132-141`).
    #[test]
    fn the_secretkey_fixture_carries_its_cited_header() {
        assert_eq!(
            u16::from_le_bytes([SECRETKEY_WIRE[0], SECRETKEY_WIRE[1]]),
            10,
            "usSize must be 10 — sizeof(sResponse_SecretKey) (FrontMsg.h:132-141)"
        );
        assert_eq!(
            SECRETKEY_WIRE[2], 0x00,
            "ucChk is the reserved byte and travels zero"
        );
        assert_eq!(
            u16::from_le_bytes([SECRETKEY_WIRE[3], SECRETKEY_WIRE[4]]),
            OP_SECRETKEY,
            "usCmd must be FRONTMSG_RESPONSE_SECRETKEY = 30000 (FrontMsg.h:114)"
        );
        assert_eq!(
            SECRETKEY_WIRE[7], FRONT_KEY1,
            "payload slot 2 must be the first seed the double hands out"
        );
        assert_eq!(
            SECRETKEY_WIRE[9], FRONT_KEY2,
            "payload slot 4 must be the second seed the double hands out"
        );
    }

    /// The game double encodes with `Cipher::link_cipher()` on the assumption
    /// the client's inbound decoder is keyed with the same fixed `0x12/0x34`
    /// (`entry.rs:396-400`). Pin the constants on both sides so a change to
    /// either cannot pass unnoticed, and round-trip one frame to prove the
    /// transport this file hands the doubles is length-preserving
    /// (`cipher.rs:186-197`).
    #[test]
    fn the_link_cipher_keys_match_the_harness_constants() {
        let cipher = Cipher::link_cipher();
        assert_eq!(
            cipher.key1(),
            LINK_KEY1,
            "LINK_KEY1 must pin cipher.rs's fixed first key (0x12)"
        );
        assert_eq!(
            cipher.key2(),
            LINK_KEY2,
            "LINK_KEY2 must pin cipher.rs's fixed second key (0x34)"
        );
    }

    /// The transport transform the doubles perform preserves length and
    /// leaves `usSize` in the clear — the property the transcript's
    /// `wire_len` and every double's two-byte read head rely on
    /// (`cipher.rs:219-220`).
    #[test]
    fn the_link_encode_preserves_length_and_leaves_us_size_in_the_clear() {
        let mut sender = Cipher::link_cipher();
        let wire = encode_link(&mut sender, &HEARTBEAT_WIRE);
        assert_eq!(
            wire.len(),
            HEARTBEAT_WIRE.len(),
            "the cipher's output length equals its input length"
        );
        assert_eq!(
            &wire[..2],
            &HEARTBEAT_WIRE[..2],
            "usSize must travel in the clear (cipher.rs:219-220)"
        );
        let mut receiver = Cipher::link_cipher();
        let mut plain = vec![0u8; wire.len()];
        assert!(
            receiver.decode(&wire, &mut plain).unwrap_or(false),
            "a fresh link cipher must decode what its twin encoded"
        );
        assert_eq!(
            plain, HEARTBEAT_WIRE,
            "the round trip must return the exact plaintext image"
        );
    }
}
