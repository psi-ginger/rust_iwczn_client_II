//! Step 7.2: the tick loop (`WO-7.2.md` §6.6).
//!
//! Port of the legacy main loop (`main.cpp:1544-1636`), made injectable so
//! gate tests script exact message sequences without any Win32 type
//! crossing the seam (R5, P3-3):
//!
//! One iteration (one `while(loop)` body, `:1544`):
//!
//! 1. **Drain, at most 10 messages** - `for (i = 0; i < 10; ++i)` (`:1549`).
//!    Each pass: if a message waits (`PeekMessage(PM_NOREMOVE)`, `:1551`),
//!    take it (`GetMessage`, `:1553`) - a taken **quit** message ends the
//!    whole loop immediately by returning (`:1553-1556`) - otherwise
//!    translate and dispatch (`:1559-1573`). Our
//!    [`MessageQueue::poll`] is exactly one of those iterations:
//!    `Some(TickMessage::Other)` = a message was taken and dispatched,
//!    `Some(TickMessage::Quit)` = `GetMessage` returned 0, `None` = the
//!    queue was empty (the `else` at `:1577`).
//! 2. **Idle sleep** - on an *empty* iteration, `if (gamemode == 1)
//!    Sleep(10);` (`:1579`). Quirk kept verbatim (P3-5): `gamemode == 1`
//!    is the **server** game mode (`main.cpp:758`), and the sleep fires
//!    once per empty iteration - an always-idle queue in server mode
//!    sleeps up to 10 x 10 ms per tick, exactly as legacy. Stated, not
//!    "fixed".
//! 3. **Clock** - `CurrentClock = timeGetTime()` (`:1625`).
//! 4. **Hooks** - `NetworkOperation()` (`:1628`) then `ClientWinMain()`
//!    (`:1630`), in that order, both receiving the fresh clock. The game
//!    hook also receives the draw-skip flag (R7): when set, the legacy
//!    `GsGame` skips render+present while still updating
//!    (`Client-WinMain.cpp:206-219`).
//!
//! The quit-timer teardown block between drain and hooks
//! (`main.cpp:1584-1620`) is client-lifecycle plumbing (hop-to-quit +
//! NELO logging), not windowing: out of 7.2 scope, stated. The loop shape
//! around it is what this port reproduces.
//!
//! No `unsafe`, no wall-clock dependence: sleep, clock, and both hooks are
//! injected closures, so every test is deterministic (P3-3).

/// What [`MessageQueue::poll`] found in one drain iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickMessage {
    /// A normal message was taken and dispatched
    /// (`main.cpp:1559-1573`).
    Other,
    /// `GetMessage` returned 0 - `WM_QUIT` (`main.cpp:1553-1555`). Legacy
    /// returns from the loop at once; we stop without running hooks.
    Quit,
}

/// The message source, one legacy drain iteration per call
/// (`main.cpp:1549-1580`). `window.rs` adapts the real Win32 pump to
/// this trait; tests script sequences directly.
pub trait MessageQueue {
    /// One drain iteration: `Some` when a message was taken (and, on the
    /// Win32 side, dispatched), `None` when the queue was empty.
    fn poll(&mut self) -> Option<TickMessage>;
}

/// Loop configuration: the legacy `gamemode` test at the sleep site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TickConfig {
    /// `gamemode == 1` (`main.cpp:758`, tested at `:1579`) - the server
    /// game mode. Only then does an empty drain iteration sleep 10 ms.
    pub server_mode: bool,
}

/// Result of one tick (`WO-7.2.md` §6.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickOutcome {
    /// `true` when a quit message ended the loop - do not run another
    /// tick (`main.cpp:1553-1556`).
    pub quit: bool,
    /// Messages drained this tick, `0..=10`.
    pub drained: usize,
}

/// One full legacy tick: drain -> idle sleep -> clock -> network hook ->
/// game hook (`main.cpp:1544-1636`).
///
/// On a quit message the tick returns immediately with `quit: true` and
/// **no** hooks run - matching the legacy early `return` (`:1555`), which
/// skips the rest of the iteration.
pub fn run_tick<Q, S, C, N, G>(
    queue: &mut Q,
    config: &TickConfig,
    draw_skip: bool,
    sleeper: &mut S,
    clock: &mut C,
    network: &mut N,
    game: &mut G,
) -> TickOutcome
where
    Q: MessageQueue + ?Sized,
    S: FnMut(u32),
    C: FnMut() -> u64,
    N: FnMut(u64),
    G: FnMut(u64, bool),
{
    let mut drained = 0usize;
    for _ in 0..10 {
        // main.cpp:1549 - the drain bound is exactly 10 (WO §8 probe e).
        match queue.poll() {
            Some(TickMessage::Quit) => {
                // main.cpp:1553-1555: quit ends the loop immediately.
                return TickOutcome {
                    quit: true,
                    drained,
                };
            }
            Some(TickMessage::Other) => drained += 1,
            None => {
                // main.cpp:1577-1580: empty iteration + gamemode == 1.
                if config.server_mode {
                    sleeper(10);
                }
            }
        }
    }
    let now = clock(); // main.cpp:1625
    network(now); // main.cpp:1628
    game(now, draw_skip); // main.cpp:1630 + Client-WinMain.cpp:211-214
    TickOutcome {
        quit: false,
        drained,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Scripted queue: yields its scripted messages one per poll, then
    /// `None` forever.
    struct ScriptedQueue {
        script: RefCell<Vec<Option<TickMessage>>>,
    }

    impl ScriptedQueue {
        fn new(script: Vec<Option<TickMessage>>) -> Self {
            Self {
                script: RefCell::new(script),
            }
        }
    }

    impl MessageQueue for ScriptedQueue {
        fn poll(&mut self) -> Option<TickMessage> {
            let mut script = self.script.borrow_mut();
            if script.is_empty() {
                None
            } else {
                script.remove(0)
            }
        }
    }

    /// Recording hooks: every effect appends to one shared log so tests
    /// assert both counts and ordering without wall-clock or threads.
    #[derive(Default)]
    struct Recorder {
        log: RefCell<Vec<String>>,
    }

    impl Recorder {
        fn sleeper(&self) -> impl FnMut(u32) + '_ {
            let log = &self.log;
            move |ms| log.borrow_mut().push(format!("sleep{ms}"))
        }

        fn clock(&self) -> impl FnMut() -> u64 + '_ {
            let log = &self.log;
            move || {
                log.borrow_mut().push("clock".into());
                4242
            }
        }

        fn network(&self) -> impl FnMut(u64) + '_ {
            let log = &self.log;
            move |now| log.borrow_mut().push(format!("net{now}"))
        }

        fn game(&self) -> impl FnMut(u64, bool) + '_ {
            let log = &self.log;
            move |now, skip| log.borrow_mut().push(format!("game{now}/{skip}"))
        }

        fn snapshot(&self) -> Vec<String> {
            self.log.borrow().clone()
        }
    }

    const SERVER: TickConfig = TickConfig { server_mode: true };
    const CLIENT: TickConfig = TickConfig { server_mode: false };

    /// Probe (e) WO §8: the drain bound is exactly 10
    /// (`main.cpp:1549`). Raising it to 11 must fail this test.
    #[test]
    fn at_most_ten_messages_drain_per_tick() {
        let mut queue = ScriptedQueue::new(vec![Some(TickMessage::Other); 15]);
        let rec = Recorder::default();
        let outcome = run_tick(
            &mut queue,
            &CLIENT,
            false,
            &mut rec.sleeper(),
            &mut rec.clock(),
            &mut rec.network(),
            &mut rec.game(),
        );
        assert_eq!(
            outcome.drained, 10,
            "the for-loop runs i in 0..10 (main.cpp:1549)"
        );
        assert!(!outcome.quit, "no quit in a normal tick");

        // The next tick drains exactly the 5 that remain - the bound is
        // per tick, not global.
        let outcome = run_tick(
            &mut queue,
            &CLIENT,
            false,
            &mut rec.sleeper(),
            &mut rec.clock(),
            &mut rec.network(),
            &mut rec.game(),
        );
        assert_eq!(outcome.drained, 5, "remaining messages drain next tick");
        assert_eq!(
            rec.snapshot()
                .iter()
                .filter(|e| e.starts_with("sleep"))
                .count(),
            0,
            "drains of a non-empty queue never sleep"
        );
    }

    /// Quit ends the tick at once with no hooks
    /// (`main.cpp:1553-1555` early `return`).
    #[test]
    fn quit_ends_the_tick_without_running_hooks() {
        let mut queue = ScriptedQueue::new(vec![
            Some(TickMessage::Other),
            Some(TickMessage::Quit),
            Some(TickMessage::Other),
        ]);
        let rec = Recorder::default();
        let outcome = run_tick(
            &mut queue,
            &SERVER,
            false,
            &mut rec.sleeper(),
            &mut rec.clock(),
            &mut rec.network(),
            &mut rec.game(),
        );
        assert!(outcome.quit, "quit observed");
        assert_eq!(outcome.drained, 1, "one message before the quit");
        assert!(
            rec.snapshot().is_empty(),
            "no clock/network/game after the early return (main.cpp:1555)"
        );
    }

    /// Idle sleep: fires only on empty iterations and only in server mode
    /// (`main.cpp:1577-1580`).
    #[test]
    fn empty_drain_sleeps_ten_ms_only_in_server_mode() {
        // Server mode: an always-empty drain runs the empty branch 10
        // times - one sleep per empty iteration, legacy-verbatim.
        let mut queue = ScriptedQueue::new(vec![]);
        let rec = Recorder::default();
        let outcome = run_tick(
            &mut queue,
            &SERVER,
            false,
            &mut rec.sleeper(),
            &mut rec.clock(),
            &mut rec.network(),
            &mut rec.game(),
        );
        assert_eq!(outcome.drained, 0, "nothing to drain");
        let sleeps: Vec<_> = rec
            .snapshot()
            .into_iter()
            .filter(|e| e.starts_with("sleep"))
            .collect();
        assert_eq!(
            sleeps.len(),
            10,
            "one Sleep(10) per empty iteration x10 (main.cpp:1549,1579)"
        );
        assert!(
            sleeps.iter().all(|e| e == "sleep10"),
            "the sleep is exactly 10 ms (main.cpp:1579)"
        );

        // Client mode (gamemode != 1): the same empty queue never sleeps.
        let mut queue = ScriptedQueue::new(vec![]);
        let rec = Recorder::default();
        let _ = run_tick(
            &mut queue,
            &CLIENT,
            false,
            &mut rec.sleeper(),
            &mut rec.clock(),
            &mut rec.network(),
            &mut rec.game(),
        );
        assert!(
            rec.snapshot().iter().all(|e| !e.starts_with("sleep")),
            "gamemode != 1 skips Sleep entirely (main.cpp:1579)"
        );
    }

    /// Hook order and values: clock once, then network, then game with
    /// the same clock and the caller's draw-skip flag
    /// (`main.cpp:1625,1628,1630`).
    #[test]
    fn hooks_run_in_legacy_order_with_the_fresh_clock() {
        let mut queue = ScriptedQueue::new(vec![]);
        let rec = Recorder::default();
        let _ = run_tick(
            &mut queue,
            &CLIENT,
            true,
            &mut rec.sleeper(),
            &mut rec.clock(),
            &mut rec.network(),
            &mut rec.game(),
        );
        assert_eq!(
            rec.snapshot(),
            vec![
                "clock".to_string(),
                "net4242".to_string(),
                "game4242/true".to_string()
            ],
            "timeGetTime -> NetworkOperation -> ClientWinMain, draw-skip forwarded \
             (main.cpp:1625,1628,1630; Client-WinMain.cpp:211-214)"
        );
    }
}
