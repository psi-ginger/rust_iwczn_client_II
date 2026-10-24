use super::*;
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

/// Binds a loopback listener on an ephemeral port.
async fn local_listener() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
}

#[tokio::test]
async fn connect_to_an_open_local_port_succeeds() {
    let (_listener, addr) = local_listener().await;
    let session = Session::connect(addr).await.expect("open port");
    assert_eq!(session.peer_addr(), addr);
    assert_eq!(session.timeouts(), SessionTimeouts::default());
}

#[tokio::test]
async fn connect_to_a_closed_local_port_fails_fast_and_gracefully() {
    // Reserve a port, then release it: nothing listens there.
    let (listener, addr) = local_listener().await;
    drop(listener);

    let started = Instant::now();
    let err = Session::connect(addr).await.unwrap_err();
    assert!(
        matches!(&err, SessionError::Io(io) if io.kind() == io::ErrorKind::ConnectionRefused),
        "expected ECONNREFUSED, got {err:?}"
    );
    // Not "instant": Windows delivers loopback ECONNREFUSED only after
    // its SYN retry timing (~2 s observed on the dev host, firewall
    // dependent). The contract under test is that the refusal surfaces
    // on its own — the OS error path, not the 5 s timeout path — so
    // bound it clearly under the connect window.
    assert!(
        started.elapsed() < DEFAULT_CONNECT_TIMEOUT,
        "a refusal must surface via the OS error path, not wait out the {:?} window (took {:?})",
        DEFAULT_CONNECT_TIMEOUT,
        started.elapsed()
    );
}

#[tokio::test]
async fn connect_times_out_on_a_black_holed_address() {
    // 192.0.2.0/24 is TEST-NET-1 (RFC 5737): reserved for documentation,
    // unroutable, and verified a black hole on the dev host (SYN
    // silently dropped, no ICMP reply).
    let addr: SocketAddr = "192.0.2.1:81".parse().unwrap();
    let timeouts = SessionTimeouts {
        connect: Duration::from_millis(300),
        ..SessionTimeouts::default()
    };

    let started = Instant::now();
    let err = Session::connect_with_timeouts(addr, timeouts)
        .await
        .unwrap_err();
    match err {
        SessionError::ConnectTimeout { addr: got, timeout } => {
            assert_eq!(got, addr);
            assert_eq!(timeout, Duration::from_millis(300));
        }
        other => panic!("expected ConnectTimeout, got {other:?}"),
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(300),
        "the window must actually bound the wait ({elapsed:?})"
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "the window must actually cut the wait short ({elapsed:?})"
    );
}

#[tokio::test]
async fn read_times_out_when_the_peer_sends_nothing() {
    let (listener, addr) = local_listener().await;
    // Accept and hold the socket open without writing: silence, not EOF.
    let holder = tokio::spawn(async move {
        let (stream, _addr) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(30)).await;
        drop(stream);
    });

    let timeouts = SessionTimeouts {
        read: Duration::from_millis(300),
        ..SessionTimeouts::default()
    };
    let mut session = Session::connect_with_timeouts(addr, timeouts)
        .await
        .expect("open port");

    let started = Instant::now();
    let mut buf = [0u8; 16];
    let err = session.read(&mut buf).await.unwrap_err();
    assert!(
        matches!(
            err,
            SessionError::ReadTimeout { timeout } if timeout == Duration::from_millis(300)
        ),
        "expected ReadTimeout, got {err:?}"
    );
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(300) && elapsed < Duration::from_secs(3),
        "read window must bound the wait ({elapsed:?})"
    );
    holder.abort();
}

#[tokio::test]
async fn read_returns_the_peers_bytes() {
    let (listener, addr) = local_listener().await;
    let sender = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        stream.write_all(b"hello").await.unwrap();
        // Keep the socket alive until the reader has taken the bytes.
        tokio::time::sleep(Duration::from_millis(200)).await;
    });

    let mut session = Session::connect(addr).await.expect("open port");
    let mut buf = [0u8; 5];
    session.read_exact(&mut buf).await.expect("read");
    assert_eq!(&buf, b"hello");
    sender.await.unwrap();
}

#[tokio::test]
async fn read_exact_spans_a_fragmented_delivery_under_one_deadline() {
    let (listener, addr) = local_listener().await;
    let dribbler = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        stream.write_all(&[0x2A, 0x00]).await.unwrap(); // usSize LE
        tokio::time::sleep(Duration::from_millis(80)).await;
        stream.write_all(&[0x00, 0x30, 0x75]).await.unwrap(); // ucChk, usCmd LE
        tokio::time::sleep(Duration::from_millis(200)).await;
    });

    let timeouts = SessionTimeouts {
        // Two chunks 80 ms apart, each far inside the window: the fill
        // must succeed on the *overall* deadline, not restart per chunk.
        read: Duration::from_millis(500),
        ..SessionTimeouts::default()
    };
    let mut session = Session::connect_with_timeouts(addr, timeouts)
        .await
        .expect("open port");

    let mut header = [0u8; 5];
    session.read_exact(&mut header).await.expect("fill");
    assert_eq!(&header, &[0x2A, 0x00, 0x00, 0x30, 0x75]);
    dribbler.await.unwrap();
}

#[tokio::test]
async fn eof_is_zero_not_an_error() {
    let (listener, addr) = local_listener().await;
    let closer = tokio::spawn(async move {
        let (stream, _addr) = listener.accept().await.unwrap();
        drop(stream); // close immediately
    });

    let mut session = Session::connect(addr).await.expect("open port");
    let mut buf = [0u8; 8];
    assert_eq!(session.read(&mut buf).await.unwrap(), 0, "EOF is Ok(0)");
    closer.await.unwrap();
}

#[tokio::test]
async fn legacy_connect_window_is_selectable() {
    // The WBS default and the legacy's literal 7000 ms
    // (ITZSession.cpp:238) must both be reachable.
    assert_eq!(DEFAULT_CONNECT_TIMEOUT, Duration::from_millis(5_000));
    assert_eq!(LEGACY_CONNECT_TIMEOUT, Duration::from_millis(7_000));
    assert_eq!(LEGACY_CLOSE_THROTTLE, Duration::from_millis(600));
    assert_eq!(
        SessionTimeouts::legacy_connect().connect,
        LEGACY_CONNECT_TIMEOUT
    );
    assert_eq!(
        SessionTimeouts::default(),
        SessionTimeouts {
            connect: DEFAULT_CONNECT_TIMEOUT,
            read: DEFAULT_READ_TIMEOUT,
        }
    );
}

#[tokio::test]
async fn resolve_maps_localhost_to_loopback() {
    let addrs = resolve("localhost", 5700).await.expect("resolves");
    assert!(
        addrs.iter().any(|a| a.ip().is_loopback()),
        "localhost must resolve to a loopback address, got {addrs:?}"
    );
    assert!(addrs.iter().all(|a| a.port() == 5700));
}

#[tokio::test]
async fn no_nodelay_is_set_parity_with_the_legacy() {
    // The legacy netlib never sets TCP_NODELAY on the game/front links,
    // so the port must not either: default Nagle state is wire-faithful.
    let (_listener, addr) = local_listener().await;
    let mut session = Session::connect(addr).await.expect("open port");
    assert!(
        !session.stream().nodelay().unwrap(),
        "TCP_NODELAY must remain at the OS default (false on Windows)"
    );
}
