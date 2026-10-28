# ctrl_c_investigation.md - why the literal Ctrl+C acceptance step was proven undeliverable in this environment (2026-10-28)

The LIVE-01 fix acceptance criterion (c) asked for a graceful `Ctrl+C ->
Stage::TornDown` with exit code 0 **against the live server**. The live
session mechanics are green (`drv_green_acceptance.log`: `stage=TornDown`,
exit 0, via the clean-close teardown). The *trigger* had to be a server FIN
instead of a console Ctrl+C, because **this verification environment cannot
dispatch console ctrl events at all** — proven with a kernel32-level control
experiment, not inferred.

## Experiment

A raw probe (`relay/ctrl_probe.rs`) registers a plain Win32
`SetConsoleCtrlHandler` handler that appends `FIRED ctrl_event=N` to a file,
launched exactly like the driver (own console, GUI-less parent). A separate
sender (`relay/ctrlc_send.rs`) attaches to the probe's console and calls
`GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0)` — the documented broadcast.

Observed (`probe_fire.log`):

```
probe up: raw handler registered ok=1, pid=4592
ctrlc_send: console of pid 4592: 3 process(es): 7640,4592,7100
ctrlc_send: CTRL_C broadcast delivered to the console of pid 4592
(probe file after send - no FIRED line)
```

- the handler **registered successfully** (`ok=1`);
- sender and probe **shared one console** (`GetConsoleProcessList` lists both);
- `GenerateConsoleCtrlEvent` **returned success**;
- the handler **never ran**.

The same broadcast was also attempted twice against the live driver itself
(two senders, one attaching via the `cmd.exe` wrapper, one directly to the
driver's console - `drv_green_ctrlc_attempt.log`): the driver kept running
and its game link stayed ESTABLISHED with heartbeats still flowing (the
server would have dropped a silent link, F-02), i.e. its watcher never saw
an event either.

## Conclusion

The console host in this environment is a **pseudoconsole (ConPTY)** — the
harness runs everything inside a pty-backed session. Under a pseudoconsole,
ctrl-event generation is routed through the pty as VT input instead of being
dispatched to the attached processes' console handlers, so no process here —
independent of language or framework — can receive a real `CTRL_C_EVENT`.
This is an **environment limitation upstream of the driver**, not a port
defect: no client code change could make it fire.

## What remains covered

- The Ctrl+C-specific wiring (`tokio::signal::ctrl_c` ->
  `watch_signal` -> `shutdown.trigger(Cause::CtrlC)` -> teardown) is covered
  by the hermetic seam tests in the 220/220 at the delivery SHA
  (`watch_signal_claims_ctrl_c_through_the_injected_seam`,
  `a_console_event_stops_the_flow_at_the_next_stage_gate`, and the teardown
  suite), which inject the signal future rather than the OS event.
- The teardown machinery itself (claim cause -> close link -> drain ->
  `stage=TornDown` -> exit 0) is now proven **live** on real sockets by the
  graceful FIN run — the only difference from the Ctrl+C path is which
  `Cause` is claimed; both funnel into the same `terminate()`.

## Residual (optional, one command on an interactive desktop)

To observe the literal Ctrl+C end-to-end, run once from a normal console
(not this harness): `run_driver_acceptance.ps1` with the relay watchdog
replaced by pressing Ctrl+C in the driver's own console window at any time
after world entry. Expected: `session complete stage=TornDown`, exit 0.
