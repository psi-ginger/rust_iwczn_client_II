# Step 8.0 mutation probes (WO §12) - S4 clean clone at `fc1a7c7`

Protocol (§12): apply one semantic break to the pristine tree → run the named test → record
exit **101** and the failing assertion → `git checkout --` → porcelain CLEAN (0-byte
`*-restore-porcelain.txt`) → re-run the full gate set to the delivered baseline (371).
Raw artifacts per probe: `P<n>-mutation.diff`, `P<n>-test.log`, `P<n>.exit`,
`P<n>-restore-porcelain.txt`. Post-probe gates: `../postprobe/`.

All four probes were executed **twice**: once in the working tree during the verification
pass, and once here in the S4 clone for this recorded bundle - identical exits and assertions
both times.

## P1 - `validate`: clamp instead of refuse (D3 violated)

- **Mutation:** `src/input/validate.rs` `SendChat` arm - the over-cap `return None` replaced
  with `text.truncate(MAX_CHAT_BYTES)` then `Some(...)`. Diff: `P1-mutation.diff`.
- **Named test:** T9 `chat_flush_refuses_empty_and_over_cap_lines_without_clamping`.
- **Result:** exit **101** (`P1.exit`), `P1-test.log`:
  `assertion 'left == right' failed: E10: a 256-byte line is refused, never truncated` -
  `left: Some(Chat { mode: Normal, text: "xxx…×255" })`, `right: None`
  (the clamp silently enqueues a shortened line instead of refusing).
- **Restore:** `git checkout -- src/input/validate.rs`; porcelain 0 bytes = CLEAN.

## P2 - `translate`: the `!in_world` guard deleted (D4 violated)

- **Mutation:** `src/input/keybinds.rs` - the `if !ui.in_world { return None; }` guard and its
  comment removed, so input flows before the world is up. Diff: `P2-mutation.diff`.
- **Named test:** T6 `events_before_the_in_world_guard_are_discarded`.
- **Result:** exit **101** (`P2.exit`), `P2-test.log`:
  `assertion 'left == right' failed: E7: KeyDown { vk: 13 } before in_world must be discarded` -
  `left: Some(ChatFocus)`, `right: None`
  (a pre-world Enter focuses the chat line instead of being discarded).
- **Restore:** `git checkout -- src/input/keybinds.rs`; porcelain 0 bytes = CLEAN.

## P3 - `translate`: unknown-key arm falls through to `Some(SitToggle)` (D1/E6 violated)

- **Mutation:** `src/input/keybinds.rs` - the catch-all `_ => None` arm changed to
  `_ => Some(Intent::SitToggle)`. Diff: `P3-mutation.diff`.
- **Named test:** T5 `unknown_and_unbound_input_is_discarded_without_panic`.
- **Result:** exit **101** (`P3.exit`), `P3-test.log`:
  `assertion 'left == right' failed: E6: KeyDown { vk: 112 } must be discarded without panic` -
  `left: Some(SitToggle)`, `right: None`
  (an F1 press emits a sit toggle instead of being discarded).
- **Restore:** `git checkout -- src/input/keybinds.rs`; porcelain 0 bytes = CLEAN.

## P4 - `validate`: `SendChat` forced to `ChatMode::Secret` (D5 violated)

- **Mutation:** `src/input/validate.rs` - `mode: ChatMode::Normal` changed to
  `mode: ChatMode::Secret` (mode coercion in the hop). Diff: `P4-mutation.diff`.
- **Named test:** T11 `chat_flush_uses_the_declared_default_mode`.
- **Result:** exit **101** (`P4.exit`), `P4-test.log`:
  `assertion 'left == right' failed: E11/D5: the declared default travels uncoerced` -
  `left: Chat { mode: Secret, text: "hi" }`, `right: Chat { mode: Normal, text: "hi" }`
  (the flushed chat carries a coerced mode instead of the declared default).
- **Restore:** `git checkout -- src/input/validate.rs`; porcelain 0 bytes = CLEAN.

## Post-probe baseline

After all four restores: HEAD unchanged at `fc1a7c7c3f6ed684fdc78c3eae236347bd8dd600`,
porcelain CLEAN. Full gate re-run (`../postprobe/gate-exits.txt`):
`check=0 clippy=0 fmt=0 doc=0 test=0`, `cargo test` = **371 passed / 0 failed**
(335 lib + 11 fixtures + 25 e2e) - `../postprobe/test.log`.

**Result: probes 4/4 - each named test failed loudly at exit 101 with a pristine restore
and a green post-probe baseline (§12, DoD 13.4).**
