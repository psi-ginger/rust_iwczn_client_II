# Step 7.4 mutation probes - run on the S4 clone @ `b72ec33` (2026-11-06)

Each probe applies one semantic break to the pristine tree, runs the WO-named test, records the failure, and restores with `git checkout --`; porcelain is checked clean after every probe. All five must fail their named test at exit 101 (Adversary lane, `CONTRIBUTING_PHASE3.md` multi-agent model; `WO-7.4.md` §12). Baseline before/after: 360/360. Raw artifacts per probe: `probes/P<n>-mutation.diff`, `probes/P<n>-test.log`, `probes/P<n>.exit`, `probes/P<n>-restore-porcelain.txt` (0 bytes = CLEAN).

Clone: S4 clean clone detached at `b72ec33` (docs tip `46f3b8d` differs only by the §14 note).

## Probe 1 - `command_wait`'s `Some` arm never delivers (`pending()` instead of `recv()`)

- Target: `src/client/event_loop.rs` (`command_wait`)
- Named tests: **T1** `queued_commands_reach_the_handler_and_the_loop_keeps_running` + **T6** `queued_commands_travel_the_wire_byte_exact`
- Result (T1): exit 101 (expect 101)
  - `test client::event_loop::tests::queued_commands_reach_the_handler_and_the_loop_keeps_running ... FAILED`
  - `panicked at src\client\event_loop\tests.rs:642:5`
  - `assertion 'left == right' failed: the two queued commands must arrive in order, payload-exact (WO-7.4 9/T1)`
  - `  left: []`
  - ` right: [Order(Move { x: 100, y: 200 }), Chat { mode: Town, text: "hi" }]`
  - `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 324 filtered out`
- Result (T6, second named test, re-run under the same mutation - `probes/P1b-T6-test.log`): exit 101 (expect 101)
  - `test queued_commands_travel_the_wire_byte_exact ... FAILED`
  - `panicked at tests\e2e_loopback.rs:1205:10`
  - `the command session must complete cleanly: Stage { stage: InWorld, source: Session(Io(...ConnectionReset...)) }`
  - `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 24 filtered out`
- Restore: P1 + P1b : porcelain CLEAN

## Probe 2 - force the chat mode to `ChatMode::Normal` in `ClientCommand::execute`

- Target: `src/client/commands.rs` (`execute`, `Chat` arm)
- Named test: **T6** `queued_commands_travel_the_wire_byte_exact`
- Result: exit 101 (expect 101)
  - `test queued_commands_travel_the_wire_byte_exact ... FAILED`
  - `panicked at tests\e2e_loopback.rs:1244:5`
  - `assertion 'left == right' failed: the queued commands must reach the wire byte-exact, in order: G1 (Move) then G2 (Town chat)` (left/right transcript dumps differ at G2's mode byte: `03` expected, `01` observed)
  - `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 24 filtered out`
- Restore: probe2 : porcelain CLEAN

## Probe 3 - delete the dedicated `Ordered` arm, merge it into the blanket record

- Target: `src/client/driver.rs` (in-world dispatch match; `DispatchOutcome::Ordered { .. }` folded into the `Unhandled` pattern group)
- Named test: **T8** `an_order_echo_is_reported_and_the_next_delta_stays_local`
- Result: exit 101 (expect 101)
  - `test an_order_echo_is_reported_and_the_next_delta_stays_local ... FAILED`
  - `panicked at tests\e2e_loopback.rs:1457:10`
  - `the 241 echo must be reported by its dedicated record, not the blanket dispatch record (D5)`
  - `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 24 filtered out`
- Restore: probe3 : porcelain CLEAN

## Probe 4 - `Wake::CommandsClosed => return Ok(())` (closed channel stops the session, D3 violated)

- Target: `src/client/event_loop.rs` (wake match)
- Named test: **T2** `dropping_every_sender_never_stops_the_loop`
- Result: exit 101 (expect 101)
  - `test client::event_loop::tests::dropping_every_sender_never_stops_the_loop ... FAILED`
  - `panicked at src\client\event_loop\tests.rs:697:5`
  - `assertion 'left == right' failed: the loop must run to three ticker fires with the channel closed`
  - `  left: 0`
  - ` right: 3`
  - `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 324 filtered out`
- Restore: probe4 : porcelain CLEAN

## Probe 5 - swallow refusals: `command.execute(link).await?` replaced by `warn!(…) + continue` (D4 violated)

- Target: `src/client/driver.rs` (handler's `LoopEvent::Command` arm)
- Named test: **T7** `an_empty_chat_command_refuses_the_session_without_a_frame`
- Result: exit 101 (expect 101)
  - `test an_empty_chat_command_refuses_the_session_without_a_frame ... FAILED`
  - `panicked at tests\e2e_loopback.rs:1304:18`
  - `expected DriverError::Refused { stage: Stage::InWorld, detail }, got Stage { stage: InWorld, source: Session(Io(Os { code: 10054, kind: ConnectionReset, ... })) }`
  - (harness double also timed out first: `panicked at tests\e2e_loopback.rs:192:10: the game double must finish inside its budget: Elapsed(())` - the refusal never arrived, so the session was not closed early)
  - `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 24 filtered out; finished in 30.02s`
- Restore: probe5 : porcelain CLEAN

## Post-probe pristine check

- Restores: all five `probes/P<n>-restore-porcelain.txt` are 0 bytes (empty `git status --porcelain`) - tree byte-identical to the gated `b72ec33`; `git rev-parse HEAD` still `b72ec33`, `git diff HEAD` empty.
- Full gate re-run on the restored tree: `check=0 clippy=0 fmt=0 doc=0 test=0`, **360 passed / 0 failed** (324+11+25), fmt 0 bytes, doc 0 warnings - `postprobe/`.
- All five probes failed their named test loudly at exit 101; P1 additionally covers its second named test T6 (exit 101).
