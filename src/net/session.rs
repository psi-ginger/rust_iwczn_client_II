//! Async TCP session — the bounded-transport half of the legacy `ITZSession`.
//!
//! Legacy source: `NetworkLib/ITZNetLib/ITZSession.{h,cpp}`. What is *not*
//! here is the legacy's IOCP machinery (overlapped `WSARecv`/`WSASend`, the
//! 20 ms worker, the pooled session objects): all of it exists to turn a
//! blocking Winsock socket into an event loop. A port with one tokio task
//! per connection needs none of it — this module is the transport: bounded
//! connect, bounded read, and the socket itself for the framing layer
//! (Step 2.2/2.3) to build on.
//!
//! # Timeouts
//!
//! | Constant | Source | What it guards |
//! |---|---|---|
//! | [`DEFAULT_CONNECT_TIMEOUT`] (5 s) | `CONTRIBUTING.md` Step 2.1 | outbound `connect()` → give up |
//! | [`DEFAULT_READ_TIMEOUT`] (5 s) | `CONTRIBUTING.md` Step 2.1 | one read call → give up |
//! | [`LEGACY_CONNECT_TIMEOUT`] (7 s) | `ITZSession.cpp:238-243` | the legacy's own connect deadline |
//! | [`LEGACY_CLOSE_THROTTLE`] (600 ms) | `ITZSession.cpp:213-215` | close → earliest re-dial (for a future state-machine step) |
//!
//! The WBS default (5000 ms) is a *policy* number with no legacy source; the
//! legacy hard-codes 7000 ms (`GetTickCount() - m_dwConnectTick >= 7000`), the
//! same value the server port pins as `gz-net::session::CONNECT_TIMEOUT`.
//! [`SessionTimeouts::legacy_connect`] selects the legacy number for callers
//! that want bit-faithful dialing; the default follows the WBS.
//!
//! The **read** timeout is a deliberate port-added guard, not legacy
//! behaviour: the legacy's game/front links have **no** per-read timeout
//! (overlapped `WSARecv` is simply always posted; the only `SO_RCVTIMEO` in
//! the whole tree is on the HanGame billing socket,
//! `NHanGameBillingProcess.cpp:500`). The only inbound liveness control was
//! the server-side watchdog (`ITZNetIOCP.cpp:41-49`), which the client could
//! not rely on. A bounded read is what stops this port from parking a task
//! forever on a half-open socket.
//!
//! QC decision **F-02** (CAR-2026-10-24-RETRO01): the read window is
//! **decoupled from session teardown**. A [`SessionError::ReadTimeout`]
//! bounds one *wait* only — no `shutdown`, no drop, no state change; the
//! socket stays open and a later read on the same [`Session`] still delivers
//! the peer's bytes (pinned by
//! `session::tests::a_read_timeout_leaves_the_socket_usable`). Callers on the
//! **game** link must treat `ReadTimeout` as "nothing arrived yet — keep
//! waiting", never as a close signal: the game server can legitimately go
//! quiet for longer than 5 s between world ticks. Both WBS numbers (5 s
//! connect, 5 s read) are **pending live ingestion against the real Front
//! (5700) and Game (4010) servers** (QC decision F-01/F-02) — revalidate
//! there, not against synthetic loopback.
//!
//! # What the legacy `_connect` dance collapses to
//!
//! `ITZSession::_connect` (`ITZSession.cpp:206-274`) creates the socket,
//! flips it non-blocking (`ioctlsocket FIONBIO`, `:230-231`), retries
//! `connect()` on every manager tick while `WSAEWOULDBLOCK`/`WSAEINPROGRESS`/
//! `WSAEINVAL` come back (`:251-254`), gives up after 7000 ms (`:238-243`),
//! and flips back to blocking on success (`:263-264`). `tokio::net::TcpStream`
//! performs exactly that sequence internally, so the whole poll loop is one
//! awaited future here, bounded by [`tokio::time::timeout`].
//!
//! # Parity notes
//!
//! - **No `TCP_NODELAY`**: the legacy netlib never sets it (tree-wide grep:
//!   zero hits outside the billing socket), so neither does this port —
//!   default Nagle behaviour is the wire-faithful choice.
//! - **Name resolution**: `InitConnect` resolves hostnames via
//!   `gethostbyname` (`ITZSession.cpp:120-134`); [`resolve`] is its
//!   tokio equivalent for the hostname endpoints (ngrok tunnels) in
//!   `src/config.rs`.
//! - Session *states* (`NONE`/`CONNECTED`/`ESTABLISHED`/`DISCONNECTED`) and
//!   the reconnect guard belong to a later step's state box, not the
//!   transport.

use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::time::timeout;

/// Default bounded-`connect()` window (`CONTRIBUTING.md` Step 2.1).
///
/// For the legacy's own 7 s number use [`LEGACY_CONNECT_TIMEOUT`] or
/// [`SessionTimeouts::legacy_connect`].
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_millis(5_000);

/// Default bounded-read window (`CONTRIBUTING.md` Step 2.1).
///
/// **F-02 (QC decision):** this window bounds one wait, *not* the session —
/// a [`SessionError::ReadTimeout`] never closes the socket, and the game
/// link's receive loop must not treat it as teardown (module docs,
/// *Timeouts*). Pending live ingestion against the real Front (5700) and
/// Game (4010) servers (F-01/F-02).
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_millis(5_000);

/// `GetTickCount() - m_dwConnectTick >= 7000` — the legacy's connect
/// deadline (`ITZSession.cpp:238-243`); identical to the server port's
/// `gz-net::session::CONNECT_TIMEOUT`.
pub const LEGACY_CONNECT_TIMEOUT: Duration = Duration::from_millis(7_000);

/// `abs( (int)(GetTickCount() - m_dwCloseTick) ) < 600` — how soon a closed
/// session may be re-dialed (`ITZSession.cpp:213-215`); identical to the
/// server port's `gz-net::session::CLOSE_THROTTLE`. Carried here as the
/// wire-truth citation for the future state-machine step; the transport
/// itself does not dial twice.
pub const LEGACY_CLOSE_THROTTLE: Duration = Duration::from_millis(600);

/// Why a session operation failed.
#[derive(Debug, Error)]
pub enum SessionError {
    /// The bounded connect window elapsed — the legacy's 7000 ms cutoff
    /// (`ITZSession.cpp:238-243`), made configurable.
    #[error("connect to {addr} timed out after {timeout:?}")]
    ConnectTimeout {
        /// Address that never completed its handshake.
        addr: SocketAddr,
        /// Window that elapsed.
        timeout: Duration,
    },
    /// The bounded read window elapsed with no bytes. Port-added guard; see
    /// the module docs.
    #[error("read timed out after {timeout:?}")]
    ReadTimeout {
        /// Window that elapsed.
        timeout: Duration,
    },
    /// The OS refused, reset, or otherwise failed the call.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Per-session timeout configuration.
///
/// [`Default`] follows the WBS (5 s / 5 s); the legacy connect window is
/// available via [`SessionTimeouts::legacy_connect`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionTimeouts {
    /// Outbound `connect()` → [`SessionError::ConnectTimeout`].
    pub connect: Duration,
    /// One read → [`SessionError::ReadTimeout`].
    pub read: Duration,
}

impl Default for SessionTimeouts {
    fn default() -> Self {
        Self {
            connect: DEFAULT_CONNECT_TIMEOUT,
            read: DEFAULT_READ_TIMEOUT,
        }
    }
}

impl SessionTimeouts {
    /// The legacy's literal connect window (7 s) with this port's read bound.
    #[must_use]
    pub const fn legacy_connect() -> Self {
        Self {
            connect: LEGACY_CONNECT_TIMEOUT,
            read: DEFAULT_READ_TIMEOUT,
        }
    }
}

/// One TCP connection with bounded connect and read windows.
///
/// Not a state machine: it is CONNECTED by construction and carries no
/// cipher or framing. Steps 2.2/2.3 attach the codec via [`Session::stream`];
/// the session/state-box step owns lifecycle policy.
#[derive(Debug)]
pub struct Session {
    stream: TcpStream,
    peer: SocketAddr,
    timeouts: SessionTimeouts,
}

impl Session {
    /// Connects to `addr` with [`SessionTimeouts::default`] windows.
    ///
    /// # Errors
    ///
    /// [`SessionError::ConnectTimeout`] when the handshake does not complete
    /// within [`DEFAULT_CONNECT_TIMEOUT`]; [`SessionError::Io`] when the OS
    /// refuses or resets (e.g. `ConnectionRefused` on a closed local port —
    /// returned immediately, without waiting out the connect window).
    pub async fn connect(addr: SocketAddr) -> Result<Self, SessionError> {
        Self::connect_with_timeouts(addr, SessionTimeouts::default()).await
    }

    /// Connects to `addr` with explicit timeouts.
    ///
    /// # Errors
    ///
    /// As [`Session::connect`], but bounded by `timeouts.connect`.
    pub async fn connect_with_timeouts(
        addr: SocketAddr,
        timeouts: SessionTimeouts,
    ) -> Result<Self, SessionError> {
        let stream = timeout(timeouts.connect, TcpStream::connect(addr))
            .await
            .map_err(|_| SessionError::ConnectTimeout {
                addr,
                timeout: timeouts.connect,
            })??;
        Ok(Self {
            stream,
            peer: addr,
            timeouts,
        })
    }

    /// The dialed address (not `stream.peer_addr()`: stable across NAT
    /// remapping and what the caller asked for).
    #[must_use]
    pub const fn peer_addr(&self) -> SocketAddr {
        self.peer
    }

    /// The configured windows.
    #[must_use]
    pub const fn timeouts(&self) -> SessionTimeouts {
        self.timeouts
    }

    /// Reads up to `buf.len()` bytes, bounded by [`SessionTimeouts::read`].
    ///
    /// `Ok(0)` means the peer closed the connection (EOF).
    ///
    /// # Errors
    ///
    /// [`SessionError::ReadTimeout`] when no byte arrives within the read
    /// window; [`SessionError::Io`] on socket errors.
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, SessionError> {
        let window = self.timeouts.read;
        timeout(window, self.stream.read(buf))
            .await
            .map_err(|_| SessionError::ReadTimeout { timeout: window })?
            .map_err(SessionError::Io)
    }

    /// Fills `buf` exactly, bounded by [`SessionTimeouts::read`] **for the
    /// whole fill** — the deadline covers every chunk, not each read call, so
    /// a peer dribbling one byte per window cannot stall the task forever.
    ///
    /// # Errors
    ///
    /// [`SessionError::ReadTimeout`] if the fill does not complete within the
    /// read window; [`SessionError::Io`] on socket errors or EOF mid-fill.
    pub async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), SessionError> {
        let window = self.timeouts.read;
        timeout(window, self.stream.read_exact(buf))
            .await
            .map_err(|_| SessionError::ReadTimeout { timeout: window })?
            .map_err(SessionError::Io)?;
        Ok(())
    }

    /// The live socket for the framing layer (Step 2.2/2.3 own writes and
    /// cipher application; this transport deliberately has no write path of
    /// its own yet).
    #[must_use]
    pub fn stream(&mut self) -> &mut TcpStream {
        &mut self.stream
    }
}

/// Resolves `(host, port)` like the legacy `InitConnect` name branch
/// (`ITZSession.cpp:120-134`, `gethostbyname`), for hostname endpoints
/// (e.g. ngrok tunnels).
///
/// # Errors
///
/// [`io::Error`] from the resolver when the name does not resolve.
pub async fn resolve(host: &str, port: u16) -> io::Result<Vec<SocketAddr>> {
    tokio::net::lookup_host((host, port))
        .await
        .map(|iter| iter.collect())
}

#[cfg(test)]
mod tests;
