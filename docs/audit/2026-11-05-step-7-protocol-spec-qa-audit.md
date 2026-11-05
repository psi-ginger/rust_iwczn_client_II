# Step 7 Modern Rust Protocol Spec (WO Rev 2) — Pre-Implementation QA Audit

**Date:** 2026-11-05
**QA auditor (`psi-ginger`), QC lane**
**Target:** `step/7.3-in-world-actions` @ `e32f481` (docs-only, 1 commit over `main` @ `81427d2`, merge-base = `main` tip)
**File under audit:** `docs/agents/WO-step7-in-world-actions.md` (Rev 2 — lean, 446 lines)
**Spec author:** `goonzu-rs` (S1 satisfied: author ≠ signer)
**Verdict:** **APPROVED — Pre-Implementation Approval issued.** All 10 golden frames (A–I) reproduce byte-exactly; all 9 opcode values, all payload sizes and `usSize` fields, and all load-bearing legacy citations verified; **zero byte-offset, size, or opcode discrepancies found.** Four non-blocking advisories on the §8.4 mirror checklist (§5) — clarifications for the implementer, not corrections to the wire spec.

## 1. Scope of this audit

Four checklist items: (1) wire/lean-layout sanity for the 6 outbound intents + `RESPONSE_ORDER` 241; (2) independent recalculation of Golden Frames A–I against §8.3 with `usSize` cross-checks; (3) review of the §8.4 `goonzu-rs` mirror checklist for desync risks; (4) sign-off or exact corrections. Method: every claim re-derived from first principles (byte construction in a fresh script; enum arithmetic over the legacy headers; repo citations read at the cited lines) — never from the WO's own tables (S5).

## 2. Wire protocol & lean layout sanity — PASS

Frame contract (§8.1.2) verified against **both** endpoints of the record:

| Claim | Port side | Legacy side | Result |
|---|---|---|:---:|
| 5-byte header `usSize(2) \| ucChk(1) \| usCmd(2)`, LE | `src/protocol/header.rs:45` (`HEADER_SIZE = 5`), `:129-149` (`WireHeader`, `new()` sets `integrity: 0`, `size = HEADER_SIZE + payload_len`) | `GoonZu_Source/NetworkLib/ITZNetLib/itzbase.h:63-68` `struct sPacketHeader {UI16 usSize; UI08 ucChk; UI16 usCmd;}` under `#pragma pack(1)` → `sizeof = 5` | ✅ |
| `usSize = 5 + payload.len()` | `header.rs:148-155` | `MsgType.h:2967` `clHead.usSize = sizeof(sPacketHeader) + datasize` | ✅ |
| `ucChk = 0` on emit | `WireHeader::new` zeroes it ("matching the server's `Frame::new`") | — | ✅ |
| Little-endian scalars throughout | `header.rs` module contract; host `BitConverter` confirmed LE (`300 → 2C 01`) | `to_le_bytes` mandated §8.1.1 | ✅ |
| No legacy padding / no transmute | payloads built field-wise (§8.1.5); none of the 7 layouts contains a pad byte | D4 strips all 4 order-struct pads | ✅ |
| Frame-delimited chat (no in-payload length, no NUL) | length = frame `usSize`; cap = typed refusal (D6) | legacy `siDataSize`/`TCHAR[256]`/NUL at `MsgType-System.h:1100-1115` — correctly retired | ✅ |
| Empty payloads legal (Quit/Front `usSize = 5`) | precedent: heartbeat sends `&[]` (`heartbeat.rs:234`) | `PACKET_EMPTY` macro `MsgType.h:14` | ✅ |
| `MAX_CHAT_BYTES = 255` → max frame 261 B, far under `MAX_FRAME_SIZE` 64992 (`header.rs:52`) | — | — | ✅ |

Per-intent layouts (§8.2): all seven outbound shapes are minimal, ordered, and unambiguous — `kind u8` first (D3), variant-only tails, `mode u8 + UTF-8 text` for chat, `village u16`, `kind u16`, and `char_unique u32 | kind u8 | variant` for the inbound 241. The declared payload-B / `usSize` columns are arithmetically consistent for every row (`5+payload`): 10/10/6/`6+n`/5/5/7/7/14/10 — **10/10 OK** (recomputed independently).

## 3. Golden Frames A–I — ALL VERIFIED (byte-exact)

Each frame was rebuilt from first principles in a fresh script (header = `usSize LE | 0x00 | cmd LE`, payload assembled field-wise from the §8.3 inputs) and compared byte-for-byte against the spec's hex strings:

| Golden | Input | Built (independent) | Spec §8.3 | Equal | `usSize`==len | `ucChk`=0 | cmd |
|---|---|---|---|:---:|:---:|:---:|---:|
| A Order/Move | x=100, y=200 | `0A 00 00 F0 00 00 64 00 C8 00` | same | ✅ | 10==10 | ✅ | 240 |
| B Order/Attack | target 12345 | `0A 00 00 F0 00 01 39 30 00 00` | same | ✅ | 10==10 | ✅ | 240 |
| C Order/SitDown | — | `06 00 00 F0 00 02` | same | ✅ | 6==6 | ✅ | 240 |
| D Chat | Normal(1), `"hi"` | `08 00 00 75 00 01 68 69` | same | ✅ | 8==8 | ✅ | 117 |
| E Quit | empty | `05 00 00 6B 00` | same | ✅ | 5==5 | ✅ | 107 |
| E Front | empty | `05 00 00 6D 00` | same | ✅ | 5==5 | ✅ | 109 |
| F Warp | village 3 | `07 00 00 92 00 03 00` | same | ✅ | 7==7 | ✅ | 146 |
| G NPCPOS | kind 5 | `07 00 00 56 14 05 00` | same | ✅ | 7==7 | ✅ | 5206 |
| H RESPONSE_ORDER/Move | char 12345, x=100, y=200 | `0E 00 00 F1 00 39 30 00 00 00 64 00 C8 00` | same | ✅ | 14==14 | ✅ | 241 |
| I RESPONSE_ORDER/SitDown | char 12345 | `0A 00 00 F1 00 39 30 00 00 02` | same | ✅ | 10==10 | ✅ | 241 |

**10/10 byte-identical; every declared `usSize` equals the actual frame length; every `ucChk` is 0.** Hex spot-derivations: `100=0x0064`, `200=0x00C8`, `12345=0x3039` (LE `39 30 00 00`), `117=0x0075`, `107=0x006B`, `109=0x006D`, `146=0x0092`, `5206=0x1456` (LE `56 14`), `240=0x00F0`, `241=0x00F1`, ASCII `h=0x68`, `i=0x69` — all correct. The refusal fixtures (§8.3 tail) are internally consistent with D8's two-stage gate: stage 1 `len < 5 → Truncated{5}`; known kind exact (`Move/Attack 9`, `SitDown 5`) with `8 → Truncated{9}` / `10 → TrailingBytes{9}`; unknown kind before the length pass; chat `"" → EmptyChat`, `256 B → ChatTooLong{256, 255}` (256 > 255 by D6's strict `>`).

## 4. Opcode & legacy-citation verification (D1's foundation)

All nine opcodes in D1 proven numerically from `GoonZuWorld/CommonLogic/MsgType.h`'s `eGameNetworkMsg` enum (positional arithmetic from explicit anchors, not from the WO):

| Opcode | Legacy symbol | Proven at | Result |
|---:|---|---|:---:|
| 240 | `GAMEMSG_REQUEST_USERORDER` | anchor `= 200` at `:220`, entry count → **240** at `:289` | ✅ |
| 241 | `GAMEMSG_RESPONSE_USERORDER` | next entry → **241** at `:290` | ✅ |
| 107 | `GAMEMSG_REQUEST_HOPETOQUIT` | anchor `= 100` at `:56` → **107** at `:68` | ✅ |
| 109 | `GAMEMSG_REQUEST_HOPETOFRONT` | → **109** at `:71` | ✅ |
| 117 | `GAMEMSG_REQUEST_CHAT` | → **117** at `:86` (118 at `:87`) | ✅ |
| 146 | `GAMEMSG_REQUEST_WARPVILLAGE` | → **146** at `:144` | ✅ |
| 5206 | `GAMEMSG_REQUEST_NPCPOS` | anchor `= 5200` at `:1411` → **5206** at `:1420` (5207 at `:1421`, matching §3.3's inbound) | ✅ |
| 200/215/106/100 | CHARUPDATE / PERSON_IP / SERVERINFO / HEARTBEAT | anchors and counts at `:220`/`:252`/`:66`/`:56` — matches port consts | ✅ |
| 267 | `GAMEMSG_REQUEST_HIT` (dead, §3.3) | enum value **267** at `:336` | ✅ |

Load-bearing semantic citations all read at the cited lines:

- **`ORDER_*` table `CommonHeader.h:517-535`** — 19 values (`ORDER_WAIT 1` … `ORDER_SPECIAL_ACTION 19`); `ORDER_MOVE 4` (`:520`), `ORDER_ATTACK 5` (`:521`), `ORDER_SITDOWN 17` (`:533`), `ORDER_ATTACK_CLIENT 18` (`:534`) — exactly §8.2's semantic map. ✅
- **`CHATMODE_*` `MsgType-System.h:1085-1097`** — the spec's 12-value enum is an **exact** match, including the intentional gap at 7 and `SYSTEM = 8` at `:1091`. ✅
- **24-byte legacy `cltOrderData` (§9)** — `CommonHeader.h:1495-1530`: `SI08 siOrder + SI08 siHostMode + pad(2) + SI32 siPara1 + SI16 siPara2/3/4 + pad(2) + DWORD dwPushedTime + SI32 CharUnique` = **24 bytes exactly**, matching §9 field-for-field. ✅
- **Superword** `siPara1 = x | (y << 16)` at `order.cpp:148`; `MakeOrder` at `:131`; `siHostMode = BY_PLAYER` at `:172`; `BY_PLAYER = 0` at `CommonHeader.h:660`; 10-bit `uiX/uiY` packing at `:1974-1975`. ✅
- **`SendCharUpdate` stub** `GoonZuWorld/Client/Client.h:910` — `{return FALSE;}` verbatim. ✅
- **Dead 267** — `char-attack.cpp:2100-2109` send site inside `/* … else*/`. ✅
- **Target resolution (T10's authority)** — `Client-SystemNPC.cpp:546-553`: `CharUnique == MyCharUnique → 1`, else table lookup — the `Local/Nearby/Unmatched` triple is a faithful port. ✅
- **Neighbour echo (§8.4.4)** — `char-order.cpp:118` builds `GAMEMSG_RESPONSE_USERORDER` and `SendNeighbourMsg` at `:119`. ✅
- **Chat clamp/guard (D6)** — `StringCchCopy(…, 256, …)` at `MsgType-System.h:1110`, empty-guard `Client.cpp:3315`. ✅
- **Legacy chat size (§9)** — `siDataSize = 2+2+4+len+1` at `:1115` → `8+len+1` ✓; modern `1+len` saves exactly **8 B** ✓.
- **§9 reduction arithmetic** — Move/Attack −79% (19/24), SitDown −96% (23/24), Warp −50%, 241 echo −62% (15/24): all correct. ✅

## 5. §8.4 cross-repo mirror checklist — sound core, 4 non-blocking advisories

The checklist covers the right risks: shared goldens as the F-07 second source on **both** sides with drift = a finding (§8.4.3), sender identity structurally taken from the connection (D4 makes payload spoofing impossible), map-bounds/village-count validation, strict UTF-8, and the invariant that 241 never displaces opcode 200 as position truth. Nothing in it contradicts the wire spec. Four clarifications would remove residual ambiguity for the server implementer — **none changes any byte**:

- **Q-1 (recommend recording before the server mirror lands).** Duty 2 says "`kind`/`mode` known-or-refuse", but only `OrderKind` (0/1/2) and `ChatMode` (§8.2) are enumerated. `REQUEST_NPCPOS`'s `kind: u16` has **no valid-value set anywhere in the WO** (legacy `siKind` semantics were never extracted — §3.3 lists inbound 5207 as "not extracted"). A server implementer must either invent a set — risking a mismatch against whatever the client later assumes — or pass values through. Fix: one line scoping the duty to `OrderKind`/`ChatMode` and declaring NPCPOS `kind` an opaque `u16` (server-defined range or pass-through). Without it, "client sends kind=5, server refuses" is a possible mirror desync.
- **Q-2.** Refusal *behavior* on the server is unspecified (silent drop vs disconnect vs error frame). The client cannot observe any of it (§6 admits no new inbound opcodes beyond 241), so no desync is possible — but stating "any refusal behavior is conformant; the client cannot observe it" prevents a future finding against the server.
- **Q-3.** The 241 broadcast scope ("neighbour visibility") is not defined (legacy used `SendNeighbourMsg`; radius/cell/interest-set unspecified). Client is report-only (T11) so no desync; mark the scope "server-defined, client-agnostic" or cite the legacy rule to keep the §1 semantics-of-record honest.
- **Q-4 (notation only).** §8.2 declares the wire field `char_unique u32` while §8.4's outcome and the whole repo use `i32` (`i32_at`, `entry.rs:252` `i32::from_le_bytes`, `dispatcher.rs:422/661`). Four LE bytes are bit-identical under either type for all valid ids, so no desync is possible; annotate §8.2 as "`i32` (4 LE bytes, repo convention)" so the two halves of the contract read the same.

Gate-risk check (beyond checklist): the new `pub` API (`encode_*`, `send_*`, `EMPTY_BODY`) is unused until the Phase-3 driver WO, but `src/lib.rs` exposes `pub mod game` — externally reachable items cannot trip `dead_code` under `clippy -D warnings`, so Phase 1's gates are safe as specified.

## 6. Repo-fact spot checks (claims the WO rests on)

| Claim | Verified |
|---|---|
| Inbound surface `1, 3, 5, 6, 11, 106, 118, 200, 215, 1102`; dispatcher arms 6/11/118/200/215 at `:867-885`, `other => Unhandled` at `:885` | ✅ all consts exist; match block read line-by-line |
| Outbound surface `2, 4, 100, 1101` at `auth/login.rs:304`, `world_entry.rs`, `heartbeat.rs:233` (`REQUEST_HEARTBEAT = 100` at `:158`) | ✅ |
| `GameLink::send` gate `entry.rs:530-535`, `NotHandshaken` before cipher, zero bytes | ✅ (T13's `:524-533` covers the doc + gate) |
| `ApplyTarget` / `DispatchOutcome` exist (`dispatcher.rs:616`, `:634`) — §8.4's `Ordered` reuses them | ✅ |
| "the 26 dispatcher tests" must pass unchanged | ✅ counted: 26 |
| Harness `local_listener`/`frame`/`encode_link` in `src/game/test_support.rs` (`:18`/`:28`/`:43`) | ✅ |
| `world_entry/tests.rs:178-189` citation table, `:740-860` round trips, `:924` zero-bytes cf. | ✅ |
| Baseline "337/337 at `main` @ `81427d2`" | ✅ re-derived on this host at the Step 6 merge (fmt 0, 304+11+22, 5 ignored) |
| WO/branch cut: docs-only, merge-base = `main` tip `81427d2` | ✅ 1 file added, no `src/**` byte exists on the branch yet (definition-before-implementation holds) |

## 7. Approval

**Pre-Implementation Approval — GRANTED.** The Step 7 Modern Rust Protocol Spec (Rev 2) is sound as the shared client ↔ `goonzu-rs` wire contract: all seven message layouts are lean and padding-free, every scalar is little-endian, framing/`usSize`/`ucChk` match both the port's codec and the legacy header, and **Goldens A–I are byte-exact under independent recalculation with all `usSize` fields correct (10/10)**. No byte offset, size, or opcode discrepancy was found, so no correction is issued.

The implementer may proceed to **Phase 1** (`src/game/action.rs`, T1–T7/T13–T14) on a branch cut from `main` @ `81427d2` per the WO's file plan. Advisories Q-1–Q-4 (§5) are non-blocking: record Q-1's NPCPOS-`kind` scoping before the `goonzu-rs` mirror lands (DoD item 7 already files a mirror gap as a finding if it hasn't), and carry Q-2–Q-4 as one-line clarifications in any Rev 2.1 touch — none may alter §8.3's goldens without a WO revision (§8.4.3).

**Sign-off:** `psi-ginger` · QC lane · 2026-11-05 · evidence: this report's cited `file:line` reads, all reproducible from `e32f481` and `GoonZu_Source/`.
