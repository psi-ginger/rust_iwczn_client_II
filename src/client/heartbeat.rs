//! Step 5.2 - the in-game heartbeat / keepalive (`CONTRIBUTING.md:148-151`).
//!
//! A recurring [`Ticker`] that pushes the header-only `GAMEMSG_REQUEST_HEARTBEAT`
//! frame over the established game link: opcode [`REQUEST_HEARTBEAT`] (100),
//! `usSize` 5, **no payload**, enciphered with the negotiated session cipher,
//! never compressed. Fire-and-forget - the server answers nothing.
//!
//! # The wire: opcode 100, `usSize` 5, empty body
//!
//! The legacy build site is the in-game action `cltClient::ClientAction2()`
//! (`Client.cpp:4916`, heartbeat at `:5035-5044`):
//!
//! ```cpp
//! if(TABS( pclClient->CurrentClock - m_SentHeartBeatClock ) > 30 * 1000 )
//! {
//!     m_SentHeartBeatClock = pclClient->CurrentClock;
//!     SI32 id = 1;
//!     if(pclCM->IsValidID(id))
//!     {
//!         cltMsg clMsg(GAMEMSG_REQUEST_HEARTBEAT, 0, NULL);
//!         SendMsgToServer((sPacketHeader*)&clMsg);
//!     }
//! }
//! ```
//!
//! `cltMsg(type, 0, NULL)` sets `clHead.usSize = sizeof(sPacketHeader) + 0`
//! (`MsgType.h:2957-2972`, the `usSize` assignment at `:2967`), and
//! `sPacketHeader` is 5 packed bytes - `UI16 usSize; UI08 ucChk; UI16 usCmd`
//! (`itzbase.h:58-70`, `#pragma pack(1)` at `:58`). The opcode is
//! `GAMEMSG_REQUEST_HEARTBEAT = 100` (`MsgType.h:56`); the server port
//! generates the same value from the same header (`GameMsg::RequestHeartbeat
//! = 100`, `gz-proto/src/opcodes_game.rs:52-53`, named at `:3305`).
//!
//! Pre-cipher our frame is therefore exactly five bytes:
//!
//! ```text
//! 05 00    usSize = 5, little-endian
//! 00       ucChk  - reserved: this port zeroes it (WireHeader::new); the
//!                   legacy never writes it either, and the cipher replaces
//!                   the byte before transmission (ITZEncrypt.cpp:44)
//! 64 00    usCmd  = 100, little-endian
//! ```
//!
//! **Session-ciphered, not link-ciphered.** `SendMsgToServer` grabs
//! `pGameSession->GetEncrypt()` (`Client-Winsock.cpp:174`) and calls
//! `pEncrypt->Encode` (`:195`, `:200`), after the `bGameServerSecretKeySwitch`
//! gate at `:171`. That is the same pairing [`GameLink::send`] implements
//! (`game/entry.rs:484-503`), and the same gate surfaced as
//! [`GameError::NotHandshaken`] - pinned by
//! `tests::send_before_the_handshake_is_refused_and_writes_nothing`.
//!
//! **Never compressed.** The legacy compresses only when `usSize >= 200`
//! (`Client-Winsock.cpp:178`); below that it takes the `else` branch and
//! encodes the frame as-is (`:197-201`). 5 < 200, so no LZF and no `0x8000`
//! flag. This port's [`crate::net::codec::Encoder`] never sets that flag
//! either - compression sits *above* the codec (`net/codec.rs`, module docs)
//! - so neither side compresses this frame and the flag never appears on it.
//!
//! # The cadence: 10 s here, 30 s in the legacy (a documented divergence)
//!
//! | Source | Interval | Citation |
//! |---|---|---|
//! | This step's specification | **10 s** | `CONTRIBUTING.md:150`, "10-second recurring ticker firing keepalive pings" |
//! | The legacy client | **30 s** | `Client.cpp:5035`, `> 30 * 1000` ms, stamped into `m_SentHeartBeatClock` (`Client.h:708-710`) |
//!
//! Both are kept as named constants: [`HEARTBEAT_INTERVAL`] (what this port
//! runs) and [`LEGACY_HEARTBEAT_INTERVAL`] (what the C++ ran). The divergence
//! is deliberate and one-directional - 10 s is **strictly more frequent** than
//! 30 s, so every liveness property the legacy cadence satisfied is satisfied
//! here too; the change can only add traffic, never remove a keepalive. The
//! legacy also guards the send with `pclCM->IsValidID(1)`
//! (`Client.cpp:5038-5040`); this port's equivalent precondition is an
//! established, handshaken link, which [`GameLink::send`] enforces
//! structurally before a byte is written.
//!
//! # What the server does with it (both trees agree: nothing)
//!
//! - **Legacy:** registered in the dispatch map (`Server-Message.cpp:129`) and
//!   handled by an empty body (`DoMsg_GAMEMSG_REQUEST_HEARTBEAT`,
//!   `Server-System.cpp:520-524`) - the client says "still here" and the
//!   server replies nothing.
//! - **Port:** the same no-op arm, whose comment cites the same C++
//!   (`services/game/src/main.rs:1863-1868`), and the receive log line for
//!   this opcode is deliberately **muted** (`main.rs:1106-1112`).
//!
//! The port still *accounts* for the frame: `Registry::note_activity` runs
//! **before** dispatch on every inbound frame (`main.rs:1113-1116`), but
//! opcode 100 sits in `counts_as_activity`'s `NOT_PLAYER` table
//! (`registry.rs:1111-1114`, rationale at `:1100-1102`) precisely because it
//! is "automated keepalive" traffic the human never produced
//! (`registry.rs:501-505`). So the heartbeat does **not** reset the server's
//! AFK clock - it is received, classified as non-player, and dropped.
//!
//! There is a second automated keepalive in that same sentence - opcode 230
//! (statusinfo) on a 30 s timer (`registry.rs:503-504`) - which is a separate
//! frame and a separate step's concern; this module sends only 100.
//!
//! # What it is for: the legacy's three-minute idle check
//!
//! The frame's purpose is `cltCharServer::AutoLogOut` (`char.cpp:5141-5149`):
//! a character is logged out when `CurrentClock - LastReceveFromClientClock >
//! 3 * 60 * 1000` (`:5149`) - but only once a receive has armed that clock,
//! because `LastReceveFromClientClock == 0` short-circuits the check
//! (`:5146`), and `SetLastReceveFromClientClock` (`:9954-9957`) is what a
//! receive stamps. A periodic heartbeat keeps the stamp fresh so the window
//! never expires. The port reads the mechanism the same way: "the only
//! liveness control on the game link is `cltCharServer::AutoLogOut`'s
//! three-minute idle check (`char.cpp:5140-5149`)"
//! (`crates/gz-net/src/session.rs:51-53` of `psi-ginger/rust_Iwcgz_server`).
//!
//! This is **not** the port's `KEEPALIVE_IDLE` / `KEEPALIVE_INTERVAL`
//! (60 s / 10 s, `main.rs:205-213`): those configure OS **TCP keepalive**
//! probes whose job is to notice a peer that vanished behind a NAT without a
//! FIN, and the port states outright that "an established session is never
//! idle-kicked (the legacy wasn't)" (`main.rs:207`). Two distinct mechanisms,
//! never to be conflated: TCP keepalive proves a peer is *dead*; this
//! heartbeat proves to the application-layer idle check that it is *alive*.
//!
//! # Acceptance (`CONTRIBUTING.md:151`) - what is and is not observable
//!
//! > Server logs continuously record heartbeat/ping packets over 60 seconds
//! > without dropping connection.
//!
//! - **"without dropping connection"** is the property this module and Step
//!   5.1 jointly provide: a ticker that keeps firing over a link that stays
//!   open, pinned by `tests::the_loop_pushes_a_heartbeat_on_every_tick_over_an_open_connection`.
//! - **"server logs record"** cannot hold *literally* against the server port
//!   as written: it mutes exactly this opcode's log line (`main.rs:1106-1112`,
//!   "100 (heartbeat) ... muted, they dominated the recv log for no signal").
//!   What is verifiable instead is that the frames leave this client
//!   byte-exactly (`tests::the_heartbeat_is_a_header_only_frame_session_ciphered_and_uncompressed`)
//!   and that the server's receive path still runs on them (`main.rs:1116`).
//!   Surfaced here rather than papered over.
//! - The **60-second soak** itself needs a live server; per F-01/F-02 the
//!   suite is loopback, so it proves the *recurrence* with a shortened test
//!   interval instead, and the loopback peer counts the frames it receives.
//!
//! # Test addresses are synthetic (F-01/F-02)
//!
//! As on every link so far (QC decisions on CAR-2026-10-24-RETRO01): the live
//! `goonzu-rs` server is unavailable, so the suite drives a loopback peer that
//! writes the real opcode 1 handshake and reads the client's frames back with
//! `usSize` in the clear + the negotiated session cipher - the same way
//! `services/game/src/main.rs` reads them. F-07: the expectation source is
//! never this module - the pre-cipher frame image is handwritten
//! `05 00 00 64 00`, the opcode is the literal `100` from `MsgType.h:56`, and
//! the intervals are the literal `10`/`30` seconds from `CONTRIBUTING.md:150`
//! and `Client.cpp:5035`.

use crate::client::event_loop::{LoopEvent, Ticker};
use crate::game::entry::{GameError, GameLink};
use std::time::Duration;

/// `GAMEMSG_REQUEST_HEARTBEAT` = 100 - the keepalive opcode
/// (`GoonZu_Source/GoonZuWorld/CommonLogic/MsgType.h:56`; the server port's
/// generated `GameMsg::RequestHeartbeat = 100`,
/// `gz-proto/src/opcodes_game.rs:52-53`, from the same header).
pub const REQUEST_HEARTBEAT: u16 = 100;

/// This step's cadence: **10 seconds** (`CONTRIBUTING.md:150`, "10-second
/// recurring ticker firing keepalive pings over the active game session").
///
/// The legacy client sent every **30** seconds ([`LEGACY_HEARTBEAT_INTERVAL`],
/// `Client.cpp:5035`); the spec's 10 s is deliberately kept as the running
/// value because it is strictly more frequent - a one-directional divergence
/// that can only add liveness (module docs, "The cadence"). Both are far
/// inside the legacy server's three-minute `AutoLogOut` window
/// (`char.cpp:5149`).
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);

/// The legacy client's own heartbeat cadence: **30 seconds**
/// (`Client.cpp:5035`, `TABS(CurrentClock - m_SentHeartBeatClock) > 30 *
/// 1000`; the clock member is `Client.h:708-710`).
///
/// Kept as a named constant rather than deleted, so the divergence above stays
/// citable and testable; this port runs [`HEARTBEAT_INTERVAL`].
pub const LEGACY_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// The [`Ticker::name`] this step's keepalive runs under.
///
/// [`is_heartbeat`] keys on this label alone, so a caller may shorten the
/// interval (tests, or a future tuning) without the predicate going blind -
/// the *identity* of the tick is the name, not the period.
pub const HEARTBEAT_TICKER_NAME: &str = "heartbeat";

/// The ticker [`crate::client::event_loop::run`] accepts for this step:
/// [`HEARTBEAT_TICKER_NAME`] at [`HEARTBEAT_INTERVAL`].
///
/// Zero cost to build; pass it in `tickers` and route
/// [`LoopEvent::Ticked`] through [`is_heartbeat`] in the handler.
#[must_use]
pub const fn heartbeat_ticker() -> Ticker {
    Ticker::new(HEARTBEAT_TICKER_NAME, HEARTBEAT_INTERVAL)
}

/// Whether `event` is **this module's ticker firing** - the signal to send a
/// heartbeat.
///
/// Matches on [`HEARTBEAT_TICKER_NAME`] alone (module docs, the name constant),
/// so the interval can vary without changing identity, and it deliberately
/// does *not* match an inbound [`LoopEvent::Frame`] carrying opcode 100: that
/// would be a received heartbeat, not a prompt to transmit one. The server
/// never sends one anyway (`Server-System.cpp:520-524`), but the distinction
/// keeps the predicate about its actual job.
#[must_use]
pub fn is_heartbeat(event: &LoopEvent) -> bool {
    matches!(
        event,
        LoopEvent::Ticked {
            name: HEARTBEAT_TICKER_NAME,
            ..
        }
    )
}

/// Sends one heartbeat: the header-only 5-byte `GAMEMSG_REQUEST_HEARTBEAT`
/// frame (module docs, "The wire"), enciphered with the negotiated session
/// cipher by [`GameLink::send`] - the legacy's `cltMsg(100, 0, NULL)` +
/// `SendMsgToServer` (`Client.cpp:5041-5042`).
///
/// No payload is built and none is expected back: the server's handler is an
/// empty no-op (`Server-System.cpp:520-524`), so this is a pure write.
///
/// # Errors
///
/// [`GameError::NotHandshaken`] before [`GameLink::handshake`] - the legacy's
/// `bGameServerSecretKeySwitch` gate (`Client-Winsock.cpp:171`), which refuses
/// the call before the cipher is ever reached, so a refused heartbeat writes
/// nothing; then the [`GameLink::send`] write failures. A handler that
/// propagates the error stops the loop through
/// [`crate::client::event_loop::LoopError::Handler`], exactly as the legacy
/// abandons its send path on a write failure.
pub async fn send_heartbeat(link: &mut GameLink) -> Result<(), GameError> {
    link.send(REQUEST_HEARTBEAT, &[]).await
}

#[cfg(test)]
mod tests;
