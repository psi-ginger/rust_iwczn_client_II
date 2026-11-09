# evidence/8.4/live - D28 live acceptance bundle

**NOT RUN - no reachable game service.**

Per WO-8.4 section 11 (`live/` clause) and CONTRIBUTING.md:28-46: a live pass is
*evidence, not a gate* - it is never required for a step to be PASSED, and no
verification-gate item (1-6) and no section-13 item requires this bundle. Step 8.4's
verdict is therefore unaffected by its absence.

What would populate this directory when a reachable server + operator env exist
(the CONTRIBUTING.md:28-46 recipe, adapted for the acceptance vehicle):

- `README.md` (this file, replaced): env recipe, relay wiring, capture commands,
  credential policy.
- `relay/`: the copied transparent-capture project from `evidence/live/relay/`.
- `live-session.log`: the vehicle's stdout - latency table, feed results, echo
  observations, final stage.
- `latency.md`: input->enqueue per-event figures against P4-C3's <= 16 ms / < 50 ms
  lines; input->echo observed figures marked *evidence, not a budget*.
- `windowed-run.md`: the operator walkthrough of the composed binary (window opens,
  click walks, chat line flushes + opcode 118 in the debug log, Insert toggles,
  Escape cancels, X closes with stage=TornDown exit 0) - D25/D26's operator
  verification.

Vehicle test (present, ignored, never a gate):
`tests/live_acceptance.rs::live_session_walks_chats_sits_and_cancels_within_budget`
- `#[ignore]`d with a reachable-service + credential ignore message (GOONZU_GAME_ADDR,
  GOONZU_USER_ID, GOONZU_USER_PW, GOONZU_MAC_ADDRESS, GOONZU_CHAR_SLOT), confirmed
  1 ignored in both gate runs (6 ignored total).

D25/D26's Win32-side ordering (WM_ENTER_WORLD arm, window thread + ready ordering,
close_demo/trigger pairing) is likewise operator-verification-only (P4-C4) and never a
gate or a mutation probe; the not-run status of this bundle does not block the verdict
(section 11's closing clause).

recorded: 2026-11-09 (+07:00) (Verifier, S4 clone @ 19e603d)
