Mutation probes - WO-8.2 section 12 (Adversary lane), executed by the Verifier
S4 clone HEAD: 937c9ab1e6c35456fa0f2748207fa4257960d000   captured: 2026-11-07 (local +07:00)
Discipline: one semantic break on the pristine tree -> named test run -> exit 101 + output ->
git checkout -- -> porcelain CLEAN, then the full five-gate set re-run green at the delivered
baseline (WO-6.3 section 9 / 7.x / 8.0 mutation-probes.md). Probe runs carry no RUSTFLAGS
(section 12's plain named-test run). All four mutations match the section 12 table rows 1-4.

### P8.2-1 - section 12 row 1: delete the D9.1 block-checks (the `repeat_vk == Some(vk) -> None`
### early-outs in the VK_RETURN and VK_INSERT arms - auto-repeat flows again)
file: src/input/keybinds.rs
named test: held_insert_fires_once_until_released   (T12, expected: exit 101)

--- mutated diff (git diff) ---
cmd: git diff -- src/input/keybinds.rs
diff --git a/src/input/keybinds.rs b/src/input/keybinds.rs
index fb99e2e..16387a4 100644
--- a/src/input/keybinds.rs
+++ b/src/input/keybinds.rs
@@ -54,9 +54,6 @@ pub fn translate(event: InputEvent, ui: &mut UiState) -> Option<Intent> {
                 // D9.1: OS auto-repeat (repeated KeyDown, one KeyUp) must
                 // not re-fire — the legacy WM_KEYUP binding gave one action
                 // per physical press for free (E13).
-                if ui.repeat_vk == Some(VK_RETURN) {
-                    return None;
-                }
                 ui.repeat_vk = Some(VK_RETURN);
                 if ui.chat_open {
                     // E2: flush — the line travels whole or not at all;
@@ -100,9 +97,6 @@ pub fn translate(event: InputEvent, ui: &mut UiState) -> Option<Intent> {
                 // D9.1: a held Insert must enqueue one Order::SitDown per
                 // physical press, not one per OS repeat — the unguarded
                 // stream spams the wire (E13, P4-C3).
-                if ui.repeat_vk == Some(VK_INSERT) {
-                    return None;
-                }
                 ui.repeat_vk = Some(VK_INSERT);
                 if ui.chat_open {
                     // §7.3: the typing state swallows the key — the line
(exactly 6 deletions / 0 insertions - verified by count: plus=0 minus=6)

--- probe run ---
cmd: cargo test --lib held_insert_fires_once_until_released
thread 'input::tests::held_insert_fires_once_until_released' (7132) panicked at src\input\tests.rs:448:9:
assertion `left == right` failed: E13/D9.1: auto-repeat 1 must be discarded, not re-fired
  left: Some(SitToggle)
 right: None
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s
process exit: 101 (expected 101)

--- restore ---
cmd: git checkout -- src/input/keybinds.rs
git status --porcelain: (clean - no output)

### P8.2-2 - section 12 row 2: remove the D11 control filter - `line.push(ch)` unconditionally
### while chat is open
file: src/input/keybinds.rs
named test: control_characters_never_enter_the_chat_line   (T17, expected: exit 101)

--- mutated diff (git diff) ---
cmd: git diff -- src/input/keybinds.rs
diff --git a/src/input/keybinds.rs b/src/input/keybinds.rs
index fb99e2e..5368c0a 100644
--- a/src/input/keybinds.rs
+++ b/src/input/keybinds.rs
@@ -148,7 +148,7 @@ pub fn translate(event: InputEvent, ui: &mut UiState) -> Option<Intent> {
             // E15/D11: control characters never enter the line — the
             // legacy EDIT class consumed them as commands (`'\r'` is the
             // flush key, `'\x08'` the edit key), never as text.
-            if ui.chat_open && !ch.is_control() {
+            if ui.chat_open {
                 ui.line.push(ch);
                 // D9.3: a character the player actually typed ends the
                 // auto-repeat window (E13; also what keeps T1/T10's

--- probe run ---
cmd: cargo test --lib control_characters_never_enter_the_chat_line
thread 'input::tests::control_characters_never_enter_the_chat_line' (4128) panicked at src\input\tests.rs:645:9:
E15: the line stays empty after '\r'
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s
process exit: 101 (expected 101)
(the assertion is a message-style `assert!` - no left/right pair, by test design)

--- restore ---
cmd: git checkout -- src/input/keybinds.rs
git status --porcelain: (clean - no output)

### P8.2-3 - section 12 row 3: the Escape dialog-close branch forgets `ui.dialog_open = false`
### (falls through to the plain E4 signal)
file: src/input/keybinds.rs
named test: escape_closes_the_active_dialog   (T18, expected: exit 101)

--- mutated diff (git diff) ---
cmd: git diff -- src/input/keybinds.rs
diff --git a/src/input/keybinds.rs b/src/input/keybinds.rs
index fb99e2e..2687d0b 100644
--- a/src/input/keybinds.rs
+++ b/src/input/keybinds.rs
@@ -88,7 +88,6 @@ pub fn translate(event: InputEvent, ui: &mut UiState) -> Option<Intent> {
                     // E16/D10: close the active dialog
                     // (`ItemInventory.cpp:1479` `Hide()`) — still UI-only:
                     // `validate` refuses the signal (D2).
-                    ui.dialog_open = false;
                     Some(Intent::CloseDialog)
                 } else {
                     // E4: nothing to cancel — ask the UI to close (D2:

--- probe run ---
cmd: cargo test --lib escape_closes_the_active_dialog
thread 'input::tests::escape_closes_the_active_dialog' (5680) panicked at src\input\tests.rs:673:5:
E16/D10: the dialog is closed
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s
process exit: 101 (expected 101)
(message-style assertion by test design - the mutated state keeps `dialog_open == true`)

--- restore ---
cmd: git checkout -- src/input/keybinds.rs
git status --porcelain: (clean - no output)

### P8.2-4 - section 12 row 4: bind an arrow - `0x26 => Some(Intent::SitToggle)` inserted
### before the KeyDown catch-all `_` arm
file: src/input/keybinds.rs
named test: arrow_keys_remain_unbound   (T21, expected: exit 101)

--- mutated diff (git diff) ---
cmd: git diff -- src/input/keybinds.rs
diff --git a/src/input/keybinds.rs b/src/input/keybinds.rs
index fb99e2e..0e674bc 100644
--- a/src/input/keybinds.rs
+++ b/src/input/keybinds.rs
@@ -127,6 +127,7 @@ pub fn translate(event: InputEvent, ui: &mut UiState) -> Option<Intent> {
             // them, because there is nothing to hold them (D1). Pressing
             // a different key ends the auto-repeat window (D9.4 — T10's
             // `…Enter, F1, …` script depends on this clause).
+            0x26 => Some(Intent::SitToggle),
             _ => {
                 ui.repeat_vk = None;
                 None

--- probe run ---
cmd: cargo test --lib arrow_keys_remain_unbound
thread 'input::tests::arrow_keys_remain_unbound' (6268) panicked at src\input\tests.rs:753:9:
assertion `left == right` failed: E18: arrow KeyDown 0x26 must be discarded
  left: Some(SitToggle)
 right: None
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s
process exit: 101 (expected 101)

--- restore ---
cmd: git checkout -- src/input/keybinds.rs
git status --porcelain: (clean - no output)

### Process notes (disclosed, never silently dropped)

1. **Probe 4, first attempt (invalid - discarded):** the first scripted attempt built the
   mutation through a nested PowerShell array literal whose coercion dropped the trailing
   `_ => {` line from the replacement, so the mutated file lost its catch-all arm and failed
   to COMPILE - cargo exited 101 from a build error, not a test failure (no `running 1 test`
   section in the output). Detected on inspection; the file was restored (`git checkout --`,
   porcelain clean) and the probe re-run standalone with a content pre-check (mutated tree
   inspected before cargo: `0x26` arm at line 130, `_ => {` at line 131). The transcript
   quoted above is the valid second run - a real test failure with E18's tabled assertion.
2. **Probe 1, first attempt (valid break, cosmetic artifact - superseded):** the same array
   coercion appended one space to the pattern, so the mutated diff was 6 guard lines plus
   two one-space indent trims (the semantic break - both early-outs deleted - was identical,
   and the run failed with the same E13 assertion, clean restore). The probe was re-run with
   direct string replacement, producing the exact 6-deletion diff quoted above; the post-probe
   gate set (run 2) was re-run after this final probe run.

### Result
4/4 probes: named test failed at exit 101 with the tabled E-row assertion, pristine restore
after each, porcelain CLEAN after each. Post-probe full gate re-run on the restored tree
(run 2, ../postprobe/): check=0 clippy=0 fmt=0 (0 bytes) doc=0 (0 warnings) test=0 at
383 passed (347 lib + 11 asset_fixtures + 25 e2e_loopback), porcelain clean before and after.
Probe mix across section 12's four rows: the D9 auto-repeat guard (1), the D11 control
filter (2), the D10 dialog-close transition (3) and the E18 unbound-arrow refusal (4) - one
per binding decision the WO marks as failing-if-corrected (DoD 13.3/13.4).
