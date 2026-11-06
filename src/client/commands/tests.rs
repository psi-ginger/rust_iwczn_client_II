// Step 7.4 tests — the command channel's two unit invariants (WO-7.4 §9,
// T4/T5).
//
// F-01 loopback-only: T5's peer is `local_listener` on 127.0.0.1:0. F-07:
// the expectations are the WO's literals (`CommandError::Closed`,
// `ActionError::Send(GameError::NotHandshaken)`), never read back from the
// code under test — the send-gate pattern is the frozen 7.3 suite's T13
// (`src/game/action/tests.rs:298-323`), restated here for the `execute`
// *wrapper* this step adds.
use super::*;
use crate::client::test_support::local_listener;
use crate::game::entry::GameError;
use tokio::io::AsyncReadExt;

// T4 — the closed contract in the queue direction: once the session's
// receiver is gone, `CommandSender::send` reports `CommandError::Closed`
// instead of silently dropping intent (D7's synchronous send has exactly
// one failure mode).
#[test]
fn sending_after_the_receiver_is_gone_reports_closed() {
    let (sender, receiver) = channel();
    drop(receiver);

    let err = sender
        .send(ClientCommand::Order(Order::SitDown))
        .expect_err("a dropped receiver must refuse the queue");
    assert!(
        matches!(err, CommandError::Closed),
        "expected CommandError::Closed, got {err:?}"
    );
    assert_eq!(
        err.to_string(),
        "the command channel is closed (the session is over)",
        "the refusal must carry the WO-7.4 §7.1 Display verbatim"
    );
}

// T5 — the `GameError -> ActionError::Send` normalization inside
// `execute` (7.3 pinned `send_order`/`send_chat` themselves; this pins the
// new wrapper): a freshly connected, pre-handshake link refuses with
// `ActionError::Send(GameError::NotHandshaken)` and zero bytes reach the
// socket (`entry.rs:530-535`).
#[tokio::test]
async fn a_send_gate_failure_is_reported_as_an_action_send_error() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        let mut received = Vec::new();
        stream.read_to_end(&mut received).await.unwrap();
        received
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    // No handshake: the send gate refuses before the cipher is reached.
    let err = ClientCommand::Order(Order::Move { x: 1, y: 2 })
        .execute(&mut link)
        .await
        .expect_err("a pre-handshake order must refuse");
    assert!(
        matches!(err, ActionError::Send(GameError::NotHandshaken)),
        "expected ActionError::Send(NotHandshaken), got {err:?}"
    );
    let err = ClientCommand::Chat {
        mode: ChatMode::Normal,
        text: "hi".to_owned(),
    }
    .execute(&mut link)
    .await
    .expect_err("a pre-handshake chat must refuse");
    assert!(
        matches!(err, ActionError::Send(GameError::NotHandshaken)),
        "expected ActionError::Send(NotHandshaken), got {err:?}"
    );
    assert!(
        link.outbound_keys().is_none(),
        "a refused action must not key or tick the outbound cipher"
    );
    drop(link); // close so the server's read_to_end completes

    let received = server.await.unwrap();
    assert!(
        received.is_empty(),
        "a pre-handshake command must never reach the socket, got {received:02x?}"
    );
}
