Mutation probes - WO-8.0 section 12 (Adversary lane), executed by the Verifier
S4 clone HEAD: fc1a7c7c3f6ed684fdc78c3eae236347bd8dd600   captured: 2026-11-07 00:09:30 +07:00
Discipline: one semantic break on the pristine tree -> named test run -> exit 101 + output -> git checkout -- -> porcelain CLEAN (WO-6.3 section 9 / 7.x mutation-probes.md).

### P8.0-1 - section 12 row 1: validate clamps instead of refusing - the over-cap line is truncated to MAX_CHAT_BYTES and enqueued as Some(Chat{...})
file: src/input/validate.rs
named test: chat_flush_refuses_empty_and_over_cap_lines_without_clamping   (expected: exit 101)

--- mutated diff (git diff) ---
cmd: git diff -- src/input/validate.rs
diff --git a/src/input/validate.rs b/src/input/validate.rs
index cc136dc..4f92e12 100644
--- a/src/input/validate.rs
+++ b/src/input/validate.rs
@@ -36,7 +36,12 @@ pub fn validate(intent: &Intent) -> Option<ClientCommand> {
                 return None;
             }
             if line.len() > MAX_CHAT_BYTES {
-                return None;
+                let mut clamped = line.clone();
+                clamped.truncate(MAX_CHAT_BYTES);
+                return Some(ClientCommand::Chat {
+                    mode: ChatMode::Normal,
+                    text: clamped,
+                });
             }
             Some(ClientCommand::Chat {
                 mode: ChatMode::Normal,
--- probe run ---
cmd: cargo test --lib chat_flush_refuses_empty_and_over_cap_lines_without_clamping
exit: 101  (expected 101)
   Compiling goonzu_client v0.1.0 (C:\Users\DK2\AppData\Local\Temp\opencode\wo80_s4)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.88s
     Running unittests src\lib.rs (target\debug\deps\goonzu_client-389dee795ddcb436.exe)

running 1 test
test input::tests::chat_flush_refuses_empty_and_over_cap_lines_without_clamping ... FAILED

failures:

---- input::tests::chat_flush_refuses_empty_and_over_cap_lines_without_clamping stdout ----

thread 'input::tests::chat_flush_refuses_empty_and_over_cap_lines_without_clamping' (7372) panicked at src\input\tests.rs:268:5:
assertion `left == right` failed: E10: a 256-byte line is refused, never truncated
  left: Some(Chat { mode: Normal, text: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" })
 right: None
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    input::tests::chat_flush_refuses_empty_and_over_cap_lines_without_clamping

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 335 filtered out; finished in 0.00s

error: test failed, to rerun pass `--lib`
probe checks: exit 101 = True; named-test panic captured = True; 'test result: FAILED' present = True; rustc compile-error signature (error[E...]/could not compile) = NONE - the only 'error:' line is cargo's harness marker 'error: test failed, to rerun pass --lib'
--- restore ---
cmd: git checkout -- src/input/validate.rs
git status --porcelain: (clean - no output)

### P8.0-2 - section 12 row 2: translate - the !in_world -> None guard is deleted (input flows before the world is up)
file: src/input/keybinds.rs
named test: events_before_the_in_world_guard_are_discarded   (expected: exit 101)

--- mutated diff (git diff) ---
cmd: git diff -- src/input/keybinds.rs
diff --git a/src/input/keybinds.rs b/src/input/keybinds.rs
index a60739e..fc581d1 100644
--- a/src/input/keybinds.rs
+++ b/src/input/keybinds.rs
@@ -31,9 +31,6 @@ pub fn translate(event: InputEvent, ui: &mut UiState) -> Option<Intent> {
     // D4: the in-world guard replaces the legacy `GetFrame() > 40` warm-up
     // gate (`mainfunction.cpp:221,236,254,288,302,1367`) — one boolean,
     // checked first for every event (E7).
-    if !ui.in_world {
-        return None;
-    }
     match event {
         InputEvent::KeyDown { vk } => match vk {
             VK_RETURN => {
--- probe run ---
cmd: cargo test --lib events_before_the_in_world_guard_are_discarded
exit: 101  (expected 101)
   Compiling goonzu_client v0.1.0 (C:\Users\DK2\AppData\Local\Temp\opencode\wo80_s4)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.01s
     Running unittests src\lib.rs (target\debug\deps\goonzu_client-389dee795ddcb436.exe)

running 1 test
test input::tests::events_before_the_in_world_guard_are_discarded ... FAILED

failures:

---- input::tests::events_before_the_in_world_guard_are_discarded stdout ----

thread 'input::tests::events_before_the_in_world_guard_are_discarded' (3916) panicked at src\input\tests.rs:180:9:
assertion `left == right` failed: E7: KeyDown { vk: 13 } before in_world must be discarded
  left: Some(ChatFocus)
 right: None
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    input::tests::events_before_the_in_world_guard_are_discarded

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 335 filtered out; finished in 0.00s

error: test failed, to rerun pass `--lib`
probe checks: exit 101 = True; named-test panic captured = True; 'test result: FAILED' present = True; rustc compile-error signature (error[E...]/could not compile) = NONE - the only 'error:' line is cargo's harness marker 'error: test failed, to rerun pass --lib'
--- restore ---
cmd: git checkout -- src/input/keybinds.rs
git status --porcelain: (clean - no output)

### P8.0-3 - section 12 row 3: translate - the unknown-key arm falls through to Some(Intent::SitToggle) instead of None
file: src/input/keybinds.rs
named test: unknown_and_unbound_input_is_discarded_without_panic   (expected: exit 101)

--- mutated diff (git diff) ---
cmd: git diff -- src/input/keybinds.rs
diff --git a/src/input/keybinds.rs b/src/input/keybinds.rs
index a60739e..45d18e9 100644
--- a/src/input/keybinds.rs
+++ b/src/input/keybinds.rs
@@ -76,7 +76,7 @@ pub fn translate(event: InputEvent, ui: &mut UiState) -> Option<Intent> {
             }
             // E6: unknown keys are discarded outright — nothing holds
             // them, because there is nothing to hold them (D1).
-            _ => None,
+            _ => Some(Intent::SitToggle),
         },
         // §2.2-4: every binding fires on `KeyDown`; releases are accepted
         // and ignored (E6).
--- probe run ---
cmd: cargo test --lib unknown_and_unbound_input_is_discarded_without_panic
exit: 101  (expected 101)
   Compiling goonzu_client v0.1.0 (C:\Users\DK2\AppData\Local\Temp\opencode\wo80_s4)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.75s
     Running unittests src\lib.rs (target\debug\deps\goonzu_client-389dee795ddcb436.exe)

running 1 test
test input::tests::unknown_and_unbound_input_is_discarded_without_panic ... FAILED

failures:

---- input::tests::unknown_and_unbound_input_is_discarded_without_panic stdout ----

thread 'input::tests::unknown_and_unbound_input_is_discarded_without_panic' (7020) panicked at src\input\tests.rs:150:9:
assertion `left == right` failed: E6: KeyDown { vk: 112 } must be discarded without panic
  left: Some(SitToggle)
 right: None
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    input::tests::unknown_and_unbound_input_is_discarded_without_panic

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 335 filtered out; finished in 0.00s

error: test failed, to rerun pass `--lib`
probe checks: exit 101 = True; named-test panic captured = True; 'test result: FAILED' present = True; rustc compile-error signature (error[E...]/could not compile) = NONE - the only 'error:' line is cargo's harness marker 'error: test failed, to rerun pass --lib'
--- restore ---
cmd: git checkout -- src/input/keybinds.rs
git status --porcelain: (clean - no output)

### P8.0-4 - section 12 row 4: validate - SendChat forced to ChatMode::Secret (mode coercion, D5 violated)
file: src/input/validate.rs
named test: chat_flush_uses_the_declared_default_mode   (expected: exit 101)

--- mutated diff (git diff) ---
cmd: git diff -- src/input/validate.rs
diff --git a/src/input/validate.rs b/src/input/validate.rs
index cc136dc..9bedd85 100644
--- a/src/input/validate.rs
+++ b/src/input/validate.rs
@@ -39,7 +39,7 @@ pub fn validate(intent: &Intent) -> Option<ClientCommand> {
                 return None;
             }
             Some(ClientCommand::Chat {
-                mode: ChatMode::Normal,
+                mode: ChatMode::Secret,
                 text: line.clone(),
             })
         }
--- probe run ---
cmd: cargo test --lib chat_flush_uses_the_declared_default_mode
exit: 101  (expected 101)
   Compiling goonzu_client v0.1.0 (C:\Users\DK2\AppData\Local\Temp\opencode\wo80_s4)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.20s
     Running unittests src\lib.rs (target\debug\deps\goonzu_client-389dee795ddcb436.exe)

running 1 test
test input::tests::chat_flush_uses_the_declared_default_mode ... FAILED

failures:

---- input::tests::chat_flush_uses_the_declared_default_mode stdout ----

thread 'input::tests::chat_flush_uses_the_declared_default_mode' (4344) panicked at src\input\tests.rs:385:5:
assertion `left == right` failed: E11/D5: the declared default travels uncoerced
  left: Chat { mode: Secret, text: "hi" }
 right: Chat { mode: Normal, text: "hi" }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    input::tests::chat_flush_uses_the_declared_default_mode

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 335 filtered out; finished in 0.00s

error: test failed, to rerun pass `--lib`
probe checks: exit 101 = True; named-test panic captured = True; 'test result: FAILED' present = True; rustc compile-error signature (error[E...]/could not compile) = NONE - the only 'error:' line is cargo's harness marker 'error: test failed, to rerun pass --lib'
--- restore ---
cmd: git checkout -- src/input/validate.rs
git status --porcelain: (clean - no output)

## Post-probe baseline (run 2) - full gate set on the restored pristine tree
porcelain before run 2: (clean - no output)

gate exits run 2: check=0 clippy=0 fmt=0 doc=0 test=0
fmt.log bytes: 0
doc warning lines: 0

cargo test result lines:
test result: ok. 335 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 4.86s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 30.18s
test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

porcelain after run 2: (clean - no output)

## T1-T11 presence + green (DoD item 2) - one filtered run over the whole input test module
cmd: cargo test --lib input::tests::
exit: 0
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running unittests src\lib.rs (target\debug\deps\goonzu_client-389dee795ddcb436.exe)

running 11 tests
test input::tests::chat_flush_refuses_empty_and_over_cap_lines_without_clamping ... ok
test input::tests::escape_while_typing_cancels_the_line_without_a_command ... ok
test input::tests::chat_flush_uses_the_declared_default_mode ... ok
test input::tests::enter_focuses_chat_then_flushes_the_typed_line ... ok
test input::tests::escape_with_chat_closed_emits_close_dialog ... ok
test input::tests::events_before_the_in_world_guard_are_discarded ... ok
test input::tests::insert_maps_to_the_frozen_sit_down_order ... ok
test input::tests::move_to_refuses_tiles_outside_the_u16_range ... ok
test input::tests::unknown_and_unbound_input_is_discarded_without_panic ... ok
test input::tests::move_streams_never_enqueue_a_command ... ok
test input::tests::the_enqueue_path_stays_within_the_16ms_frame_budget ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 325 filtered out; finished in 0.01s

