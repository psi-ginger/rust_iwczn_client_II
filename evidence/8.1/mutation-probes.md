Mutation probes - WO-8.1 section 12 (Adversary lane), executed by the Verifier
S4 clone HEAD: cada5ed4a8ba2f2dbd58e047e121b3c11188a36e   captured: 2026-11-07 (local +07:00)
Discipline: one semantic break on the pristine tree -> named test run -> exit 101 + output ->
git checkout -- -> porcelain CLEAN, then the full five-gate set re-run green at the delivered
baseline (WO-6.3 section 9 / 7.x / 8.0 mutation-probes.md). Probe runs carry no RUSTFLAGS
(section 12's plain named-test run).

### P8.1-1 - section 12 row 1: screen_to_real drops the trailing `+ MAP_TILE_YSIZE_HALF` term (:187's constant)
file: src/input/screen.rs
named test: screen_to_tile_truncates_toward_zero_like_legacy_c   (T14, expected: exit 101)

--- mutated diff (git diff) ---
cmd: git diff -- src/input/screen.rs
diff --git a/src/input/screen.rs b/src/input/screen.rs
index fc2f6ab..9dcf5da 100644
--- a/src/input/screen.rs
+++ b/src/input/screen.rs
@@ -44,8 +44,7 @@ pub fn screen_to_real(x: i32, y: i32, center: (i32, i32), screen: &ScreenRect) ->
     // :185 — minus the view-center offset (draw_center_xy's center terms, negated).
     let realx = x - mid_x - (-center.0 * MAP_TILE_XSIZE_HALF + center.1 * MAP_TILE_XSIZE_HALF);
     // :187 — the trailing `+ MAP_TILE_YSIZE_HALF` is legacy's, kept verbatim (E15).
-    let realy = y - mid_y - (-center.1 * MAP_TILE_YSIZE_HALF - center.0 * MAP_TILE_XSIZE_HALF)
-        + MAP_TILE_YSIZE_HALF;
+    let realy = y - mid_y - (-center.1 * MAP_TILE_YSIZE_HALF - center.0 * MAP_TILE_XSIZE_HALF);
     (realx, realy)
 }

--- probe run ---
cmd: cargo test --lib screen_to_tile_truncates_toward_zero_like_legacy_c
thread 'input::tests::screen_to_tile_truncates_toward_zero_like_legacy_c' (3532) panicked at src\input\tests.rs:534:5:
assertion `left == right` failed: E15: G4 truncates toward zero like C99 (MapCommon.cpp:965-966); floor gives (-14,-2)
  left: (-14, -2)
 right: (-13, -1)
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 351 filtered out; finished in 0.01s
process exit: 101 (expected 101)

--- restore ---
cmd: git checkout -- src/input/screen.rs
git status --porcelain: (clean - no output)

### P8.1-2 - section 12 row 2: real_to_tile y-branch divisor changed XSIZE -> YSIZE (the "obvious symmetry fix")
file: src/input/screen.rs
named test: screen_to_tile_matches_hand_computed_goldens   (T13, expected: exit 101)

--- mutated diff (git diff) ---
cmd: git diff -- src/input/screen.rs
diff --git a/src/input/screen.rs b/src/input/screen.rs
index fc2f6ab..1cb2b16 100644
--- a/src/input/screen.rs
+++ b/src/input/screen.rs
@@ -56,7 +56,7 @@ pub fn screen_to_real(x: i32, y: i32, center: (i32, i32), screen: &ScreenRect) ->
 pub fn real_to_tile(realx: i32, realy: i32) -> (i32, i32) {
     (
         (realx / 2 + realy) / MAP_TILE_XSIZE_HALF,
-        (-realx / 2 + realy) / MAP_TILE_XSIZE_HALF,
+        (-realx / 2 + realy) / MAP_TILE_YSIZE_HALF,
     )
 }

--- probe run ---
cmd: cargo test --lib screen_to_tile_matches_hand_computed_goldens
thread 'input::tests::screen_to_tile_matches_hand_computed_goldens' (3532) panicked at src\input\tests.rs:507:5:
assertion `left == right` failed: E14: G1 - the rect center lands on the view center
  left: (100, 201)
 right: (100, 100)
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 351 filtered out; finished in 0.00s
process exit: 101 (expected 101)

--- restore ---
cmd: git checkout -- src/input/screen.rs
git status --porcelain: (clean - no output)

### P8.1-3 - section 12 row 3: translate's IsPtInMap gate deleted (off-screen clicks compute and proceed)
file: src/input/keybinds.rs
named test: clicks_outside_the_screen_rect_are_discarded   (T17, expected: exit 101)

--- mutated diff (git diff) ---
cmd: git diff -- src/input/keybinds.rs
diff --git a/src/input/keybinds.rs b/src/input/keybinds.rs
index d0a1270..e97e000 100644
--- a/src/input/keybinds.rs
+++ b/src/input/keybinds.rs
@@ -105,9 +105,6 @@ pub fn translate(event: InputEvent, ui: &mut UiState) -> Option<Intent> {
             let view = ui.view?;
             // E18: the IsPtInMap gate (MouseOperation.cpp:534 ←
             // Map-Client.cpp:196-206, inclusive ScreenRect bounds).
-            if !is_pt_in_map(x, y, &view.screen) {
-                return None;
-            }
             let (tile_x, tile_y) = screen_to_tile(x, y, view.center, &view.screen);
             // E19: D14's named off-map refusal (map.cpp:226 half-open; the
             // legacy gate is at UnderMouseObject.cpp:335).

--- probe run ---
cmd: cargo test --lib clicks_outside_the_screen_rect_are_discarded
warning: unused import: `is_pt_in_map`
warning: `goonzu_client` (lib test) generated 1 warning (run `cargo fix --lib -p goonzu_client --tests` to apply 1 suggestion)
thread 'input::tests::clicks_outside_the_screen_rect_are_discarded' (8156) panicked at src\input\tests.rs:604:9:
assertion `left == right` failed: E18: off-screen click (1000, 340) is discarded (IsPtInMap, Map-Client.cpp:196-206)
  left: Some(MoveTo { x: 107, y: 94 })
 right: None
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 351 filtered out; finished in 0.00s
process exit: 101 (expected 101)
(disclosed: the deleted gate's import becomes unused - a non-fatal warning under the plain
probe run; the failure itself is T17's E18 assertion as tabled)

--- restore ---
cmd: git checkout -- src/input/keybinds.rs
git status --porcelain: (clean - no output)

### P8.1-4 - section 12 row 4: TargetClass::Npc arm falls through to Some(MoveTo { .. }) instead of None
file: src/input/keybinds.rs
named test: click_on_a_friendly_npc_is_discarded_not_moved   (T21, expected: exit 101)

--- mutated diff (git diff) ---
cmd: git diff -- src/input/keybinds.rs
diff --git a/src/input/keybinds.rs b/src/input/keybinds.rs
index d0a1270..28b80a4 100644
--- a/src/input/keybinds.rs
+++ b/src/input/keybinds.rs
@@ -125,7 +125,6 @@ pub fn translate(event: InputEvent, ui: &mut UiState) -> Option<Intent> {
                 // excluded) — discarding is the declared outcome, and there
                 // is no fall-through move (legacy's bdoneswitch stops the
                 // chain there too).
-                Some(target) if target.class == TargetClass::Npc => None,
                 // D12/E20/E23: friendly player and empty land both fall into
                 // legacy's move branch (:5409; dest :5532, ORDER_MOVE :5537).
                 _ => Some(Intent::MoveTo {

--- probe run ---
cmd: cargo test --lib click_on_a_friendly_npc_is_discarded_not_moved
thread 'input::tests::click_on_a_friendly_npc_is_discarded_not_moved' (8156) panicked at src\input\tests.rs:712:5:
assertion `left == right` failed: E22: Npc class → typed None, never a MoveTo (probe 4 breaks exactly this)
  left: Some(MoveTo { x: 100, y: 100 })
 right: None
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 351 filtered out; finished in 0.00s
process exit: 101 (expected 101)

--- restore ---
cmd: git checkout -- src/input/keybinds.rs
git status --porcelain: (clean - no output)

### Result
4/4 probes: named test failed at exit 101 with the tabled E-row assertion, pristine restore
after each, porcelain CLEAN after each. Post-probe full gate re-run on the restored tree
(run 2, ../postprobe/): check=0 clippy=0 fmt=0 (0 bytes) doc=0 (0 warnings) test=0 at
387 passed (351 lib + 11 asset_fixtures + 25 e2e_loopback), porcelain clean before and after.
Probe mix: two math-parity breaks (1-2) and two refusal/discard breaks (3-4), covering the
"parity with legacy" and "refuses/discards" claim classes of gate item 6.
