# Work Order - Step 7: In-World Action & State Pipeline — Modern Lean Protocol

**Status:** `DRAFT` for Custodian alignment — definition-before-implementation.
No `src/**` byte may move before this scope is accepted.
**Date:** 2026-11-05 · **Revision:** Rev 2.1 — payload design replaced by the *Modern Rust
Protocol Spec* (§8) under the architectural direction (§2); the Rev-1 legacy-faithful
wire layouts are retired and mapped in §9. Rev 2.1 appends §15 (Future Invariant & Server Alignment Note: QA advisories Q-1..Q-4; no wire change).
**Branch:** to be cut from `main` @ `81427d2` on acceptance.
**Baseline (re-derived this session):** clean tree, `main` @ `81427d2`, all five mandatory
gates exit-0 — `cargo check --all-targets` (under `RUSTFLAGS="-D warnings"`), `cargo clippy
--all-targets -- -D warnings`, `cargo fmt --check`, `cargo test` at **337/337** (304 lib +
11 `asset_fixtures` + 22 e2e; the 5 ignored = 1 operator demo + 4 `live_integration`),
`cargo doc --no-deps` 0 warnings.
**Reference:** `README.md:101-121` (Step 5 in-world loop; Step 6 ends the documented
functional steps) · `CONTRIBUTING_PHASE3.md` (governance; append rule :185) ·
`docs/agents/WO-7.1.md`, `docs/agents/WO-step6-structure.md` (format precedents).

---

## 1. Provenance, governance, lane hats

- **Milestone.** `README.md` functional steps end at Step 6 (`README.md:115-121`); the
  Phase III rows 7.0/7.1/7.2 are the completed *asset/windowing* steps (all `PASSED`).
  "Step 7 — In-World Action & State Pipeline" is a **new protocol milestone**. Its row in
  `TASK_MATRIX.md` and the steps table (`CONTRIBUTING_PHASE3.md:179-186`) are appended by
  the **Custodian/Scribe** per `:185`; neither this WO nor the Implementer edits them.
- **Lanes:** Implementer → Verifier + **Adversary (mandatory)** — every load-bearing claim
  is "byte-exact" and "refuses malformed input" (`CONTRIBUTING_PHASE3.md:166-170`),
  dischargeable by a verified mutation probe (6.3 ratification precedent, `WO-6.3.md` §9).
- **Write guard:** only the files in §7. Everything else in `src/**`, `tests/e2e_loopback/**`,
  `tests/live_integration.rs`, `Cargo.toml`, `TASK_MATRIX.md`, `CONTRIBUTING*.md`,
  `README.md` and `GoonZu_Source/**` are READ-ONLY — a defect found there is a finding
  `F-7.N-NN`, never an edit (P3-6, `CONTRIBUTING_PHASE3.md:171-172`).
- **Source-of-record split (this revision's core governance change):**
  - **Wire format of record** = the Modern Rust Protocol Spec in **§8 of this WO**
    (client and `goonzu-rs` both mirror it; goldens are pre-computed here).
  - **Semantics of record** = the legacy tree `GoonZu_Source/` (what an *order*, a *chat
    mode*, a *warp* means) — cited `file:line` throughout, but **no legacy byte layout is
    binding** for the messages this step adds (§5/§9).

## 2. Architectural direction (psi-ginger, 2026-11-05 — binding for this WO)

1. **Modern & minimal packet design.** Strip the 2000s C++ overhead: no unused padding
   fields, no fake 24-byte structs, no legacy bloat. Send only what gameplay intents
   need — Move as a clean `x: u16, y: u16`; action/state intents as lean enums with only
   the relevant payload; system requests (Quit, Return-to-Front, Warp) as lightweight
   opcodes without redundant headers.
2. **Server alignment strategy.** The server is our own sovereign Rust server
   (`goonzu-rs`) — legacy wire inefficiencies need not be preserved. The lean protocol is
   specified once, here, so client and server mirror each other cleanly.
3. **Deliverable.** This WO carries a "Modern Rust Protocol Spec" (§8) plus the
   legacy-vs-modern comparison (§9).

## 3. Discovery record (legacy = semantics of record, not wire of record)

### 3.1 Current in-world surface of the port

| Direction | Opcodes handled today | Where |
|---|---|---|
| Inbound | 1, 3, 5, 6, 11, 106, 118, 200, 215, 1102 | `src/game/entry.rs`, `char_info.rs`, `world_entry.rs`, `dispatcher.rs:867-885` (arms 6/11/118/200/215; `other => Unhandled` at `:885`) |
| Outbound | 2, 4, 100, 1101 | `src/auth/login.rs:304`, `src/game/login.rs:233`, `src/game/world_entry.rs`, `src/client/heartbeat.rs:233` |

`GameLink::send(cmd, payload)` (`src/game/entry.rs:530-535`) is the negotiated-state send
gate; every new outbound message below rides it unchanged.

### 3.2 Legacy semantics this step inherits (why orders exist at all)

- **No client→server position packet exists** — the client's `SendCharUpdate` is a
  `return FALSE` stub (`Client.h:910`), `ServerMap.cpp:66 SendPersonMoveMsgToOthers` is a
  commented no-op; the only real `SendCharUpdate` is server-side (`Server.cpp:1654-1698`
  → `GAMEMSG_RESPONSE_CHARUPDATE` **200**). **Position authority is the server's**; the
  client applies x/y from 200 (`Client-Person-BigInfo.cpp:1139-1163`). The modern design
  keeps exactly this split (§5/D5, §8.4).
- **Movement travels as an order**: input → `cltOrder::MakeOrder` (`common/Order/order.cpp:131`)
  → `cltClient::OrderOperation` (`Client.cpp:3284-3304`) → legacy `GAMEMSG_REQUEST_USERORDER`
  (**240**, `MsgType.h:289`) → server `DoMsg_…` (`Server-Person.cpp:2700`, map
  `Server-Message.cpp:140`) → neighbour echo `GAMEMSG_RESPONSE_USERORDER` (**241**,
  `MsgType.h:290`, 8 send sites in `common/Char/CharCommon/char-order.cpp` e.g. `:118`) →
  legacy client resolves the target (`Client-SystemNPC.cpp:546-553`) and ends in
  `LetOrder(porder, orderid)` (`:689`).
- **Attack rides the order channel**: `ORDER_ATTACK` (`CommonHeader.h:521`);
  `GAMEMSG_REQUEST_HIT` (267) is dead client-side — its only send site is commented out
  (`char-attack.cpp:2100-2109`). The modern spec keeps one order channel and does **not**
  introduce 267.
- **Order kinds** live in the `ORDER_*` table (`CommonHeader.h:517-535`, 19 values);
  **chat modes** in `CHATMODE_*` (`MsgType-System.h:1085-1097`); both are inherited as
  *semantics* (§8), not as wire values.

### 3.3 Named candidates NOT in this delivery (next-step input)

| Opcode | Value | Legacy struct / note | Status |
|---|---:|---|---|
| `REQUEST_INVPICKUP` / `INVDROP` | 308 / 307 | `MsgType-Item.h:390-407` / `:66-76` | grounded; Step 8 candidate |
| `REQUEST_ITEMMOVE` / `USEITEM` | 303 / 306 | send sites exist; structs not extracted | needs extraction |
| `RIDEHORSE`/`UNRIDEHORSE`/`SITDOWN`/`REVIVE` | 5500/5502/7231/7225 | send sites `NCharHorse.cpp:566`, `MouseOperation.cpp:4133`, `NCommonMsgDlg.cpp:178` | needs extraction (SitDown *as an order kind* is in scope here, §4) |
| `REQUEST_HIT` | 267 | dead client-side (§3.2) | **do not implement** |
| inbound `INCLIFE`/`INCMANA`/`HIT`/`GETEXP`/`ITEMDROP`/`NPCPOS` | 252/253/268/270/309/5207 | not extracted | needs extraction |

## 4. Scope decision — this delivery

Two deliverables, one per direction, in the lean shapes of §8:

- **(a) Outbound action encoder batch A** — `src/game/action.rs` (new):

  | Intent | Modern name | Opcode | Lean payload |
  |---|---|---:|---|
  | Order (Move / Attack / SitDown) | `REQUEST_ORDER` | 240 | `kind: u8` + variant fields — `x: u16, y: u16` \| `target_char: u32` \| ∅ (§8.3) |
  | Chat | `REQUEST_CHAT` | 117 | `mode: u8` + UTF-8 text (frame-delimited) |
  | Quit to title | `REQUEST_QUIT` | 107 | empty |
  | Return to front | `REQUEST_FRONT` | 109 | empty |
  | Warp to village | `REQUEST_WARP` | 146 | `village: u16` |
  | System-NPC position query | `REQUEST_NPCPOS` | 5206 | `kind: u16` |

- **(b) Inbound arm: `RESPONSE_ORDER` (241)** — one additive arm in
  `src/game/dispatcher.rs`: two-stage length gate (§8.3), lean parse, the legacy target
  resolution (`Client-SystemNPC.cpp:546-553`), new `DispatchOutcome::Ordered` —
  **report-only, no `WorldChar` mutation** (§5/D8).

Out of scope: driver/event-loop wiring (§6), item actions, §3.3 candidates, and any
inbound opcode beyond 241.

## 5. Protocol decisions (modern resolutions — state, pin, do not "regress to legacy")

Each is pinned by a test that would fail if the port re-introduced the legacy shape.

- **D1 — Opcode identity is retained; only payloads are modernized.** The new messages
  keep the legacy numeric opcodes (240/241/117/107/109/146/5206). Numbers are
  identifiers, not bloat: the port's shipped consts and `goonzu-rs` already key on them,
  and keeping them preserves semantic traceability to `MsgType.h` citations. The legacy
  *payload layouts* are not inherited (§9). *(Escape hatch: a Custodian decision to move
  to a fresh opcode space changes §8.2 only; payloads and tests are unaffected.)*
- **D2 — The 5-byte frame header is transport, not bloat.** `usSize(2) | ucChk(1) |
  usCmd(2)` (`src/protocol/header.rs:45`, `:129-149`) plus the session cipher stay
  untouched. "No redundant headers" (direction §2.1) means **no in-payload
  self-descriptors**: no legacy `siDataSize`, no `cltMsg`-style wrapper duplication.
- **D3 — One order opcode, tagged enum payload.** `kind: u8` then only the variant's
  fields: `0 Move`, `1 Attack`, `2 SitDown`; `3..=255` reserved. Outbound is type-safe (a
  Rust enum cannot emit a reserved kind); inbound refuses unknown kinds **by name**
  (§8.3). This replaces the legacy pattern where one 24-byte struct carried every order
  kind's fields at once.
- **D4 — Fields stripped outright** (each with its legacy citation in §9): `siHostMode`
  (`BY_PLAYER` was a constant anyway, `CommonHeader.h:660` + `order.cpp:172`), `siPara2/3/4`
  (superseded by per-variant fields), `dwPushedTime` (the server stamps its own time),
  outbound `CharUnique` (the connection identifies the sender), all 4 alignment pad bytes
  (the encoder writes fields one by one; there is no struct transmute), and chat's
  `siSystemTxt` (folded into `ChatMode::System = 8`, `MsgType-System.h:1091`).
- **D5 — Coordinates are a clean word pair.** `x: u16 LE, y: u16 LE`, range-checked by
  the server against map bounds. The legacy superword `siPara1 = x | (y << 16)`
  (`order.cpp:148`) and its 10-bit packing (`CommonHeader.h:1974-1975`) are **retired** —
  client and server are both ours.
- **D6 — Chat is frame-delimited and strict.** Payload = `mode: u8` + UTF-8 text bytes;
  length comes from the frame's `usSize` (no `siDataSize`, no fixed 256 array, no NUL
  terminator). Caps are **refusals, never truncation** (the repo's standing invariant):
  empty text → typed `ActionError::EmptyChat`; text > `MAX_CHAT_BYTES = 255` → typed
  `ActionError::ChatTooLong { len, max }`. This deliberately diverges from the legacy
  silent clamp (`StringCchCopy`, `MsgType-System.h:1110`; empty-guard `Client.cpp:3315`).
- **D7 — Charset is UTF-8.** Both endpoints are sovereign Rust; new messages carry UTF-8
  (encoder takes `&str`; `goonzu-rs` validates strictly). Legacy CP949/`TCHAR` MBCS is
  **not** carried into new messages. (Shipped inbound 118 stays legacy-layout until a
  dedicated migration WO — §6.)
- **D8 — 241 is a two-stage gate and a report.** Stage 1: `payload.len() >= 5`
  (`char_unique: u32 | kind: u8`), else `DispatchError::Truncated`. Stage 2: known kind →
  exact expected length (`Move`/`Attack` 9, `SitDown` 5), else `Truncated`/`TrailingBytes`
  by name; unknown kind → new `DispatchError::UnknownOrderKind { kind }` **before** any
  length pass. The outcome reports; it never writes `WorldChar` — position truth stays
  opcode 200 (§3.2).
- **D9 — Second sources.** For a sovereign protocol the second source for *bytes* is the
  pre-computed golden set in §8.3 (F-07: both client and `goonzu-rs` tests assert these
  literals, never their own encoders) plus the mirror implementation. Legacy headers
  remain the second source for *semantics* only. Live-capture adjudication
  (`evidence/live/…`) applies to **shipped** opcodes, not to these new payloads.

## 6. Non-goals

- **No migration of shipped opcodes.** 2/4/6/11/100/106/118/200/215/1101/1102 keep their
  current legacy-derived payloads; modernizing them is a separate WO that must change
  `goonzu-rs` in lockstep. (Transitional state, stated honestly: new *outbound* chat is
  modern while inbound 118 remains legacy-layout.)
- **No legacy-binary interoperability.** `goonzu-rs` is the only server; a legacy C++
  server is not a compatibility target for the messages this step adds.
- No driver/event-loop wiring (`src/client/driver.rs` needs a write-guard expansion —
  separate WO); no movement simulation or local apply of orders; no position upload of any
  kind (§3.2); no `REQUEST_HIT` (267); no item-action opcodes; no inbound opcodes beyond
  241; no cipher/header changes (D2); no new dependencies; no external network, GPU,
  window, or display in any test; no edits to `TASK_MATRIX.md`, `CONTRIBUTING*.md`,
  `README.md`, `GoonZu_Source/**`.

## 7. File plan (ownership — one step, one Implementer, one file set)

| File | Role | Writer |
|---|---|---|
| `src/game/action.rs` (new) | opcode/kind/mode constants, `Order`/`ChatMode` enums, encoders, send functions (§8.3-8.4) | Implementer (single writer) |
| `src/game/action/tests.rs` (new) | Phase 1 tests (§10 T1-T7, T13-T14) | Implementer |
| `src/game/dispatcher.rs` | **additive only:** `RESPONSE_ORDER` const, `parse_response_order`, `DispatchOutcome::Ordered`, `DispatchError::UnknownOrderKind`, one match arm | Implementer (additive) |
| `src/game/dispatcher/tests.rs` | **additive only:** Phase 2 tests (§10 T8-T12) | Implementer (additive) |
| `src/game/mod.rs` | **additive only:** `pub mod action;` + one doc bullet | Implementer (additive) |
| `src/lib.rs`, `src/client/**`, `src/net/**`, `src/protocol/**`, `src/auth/**`, `src/graphics/**`, `src/game/{entry,login,char_info,world_entry,gate,test_support}.rs` | READ-ONLY | nobody |
| `tests/e2e_loopback/**`, `tests/live_integration.rs`, `Cargo.toml` | untouched | nobody |
| `TASK_MATRIX.md`, `CONTRIBUTING*.md`, `README.md`, `GoonZu_Source/**` | untouched | Custodian/Scribe (matrix rows), nobody (rest) |
| `evidence/<step>/` | clean-clone gate bundle + mutation probes | Verifier only |

"Additive only": existing arms, tests, and behaviors are never deleted or altered; the 26
dispatcher tests and every pre-existing assertion must pass unchanged.

## 8. Modern Rust Protocol Spec (the shared contract — client ↔ `goonzu-rs`)

### 8.1 Invariants

1. **Little-endian** scalar fields throughout (matches `src/protocol/header.rs`).
2. **Frame** = `usSize u16 | ucChk u8 (=0) | usCmd u16 | payload`; `usSize = 5 + payload.len()`
   (`header.rs:45`, legacy parity `MsgType.h:2967`). No other envelope; no in-payload
   length fields (D2).
3. **Opcodes** per D1 (§8.2). **Kinds** per D3. Reserved kind range `3..=255` is a typed
   refusal on receive and unrepresentable on send.
4. **Strings**: UTF-8, frame-delimited, no NUL terminator (D7); chat capped at
   `MAX_CHAT_BYTES = 255` with typed refusals (D6).
5. **Payloads are built field-wise** (`extend_from_slice(&x.to_le_bytes())` …) — never a
   `#[repr(C)]` struct transmute (there is nothing to transmute: no padding exists, D4).

### 8.2 Message table

| Rust const | Opcode | Dir | Payload (LE) | Payload B | usSize |
|---|---:|---|---|---:|---:|
| `REQUEST_ORDER` (legacy `GAMEMSG_REQUEST_USERORDER`, `MsgType.h:289`) | 240 | C→S | `kind=0 Move`: `x u16, y u16` | 5 | 10 |
| " | 240 | C→S | `kind=1 Attack`: `target_char u32` | 5 | 10 |
| " | 240 | C→S | `kind=2 SitDown`: ∅ | 1 | 6 |
| `REQUEST_CHAT` (legacy 117, `MsgType.h:86`) | 117 | C→S | `mode u8` + UTF-8 text | 1+n | 6+n |
| `REQUEST_QUIT` (legacy `HOPETOQUIT` 107, `MsgType.h:68`) | 107 | C→S | ∅ | 0 | 5 |
| `REQUEST_FRONT` (legacy `HOPETOFRONT` 109, `MsgType.h:71`) | 109 | C→S | ∅ | 0 | 5 |
| `REQUEST_WARP` (legacy `WARPVILLAGE` 146, `MsgType.h:144`) | 146 | C→S | `village u16` | 2 | 7 |
| `REQUEST_NPCPOS` (legacy 5206, `MsgType.h:1420`, anchor `=5200` at `:1411`) | 5206 | C→S | `kind u16` | 2 | 7 |
| `RESPONSE_ORDER` (legacy 241, `MsgType.h:290`) | 241 | S→C | `char_unique u32` + `kind u8` + variant payload: Move/Attack `+4`, SitDown `+0` | 9 / 5 | 14 / 10 |

Enum value tables (semantics inherited, `repr(u8)`):

```rust
enum OrderKind { Move = 0, Attack = 1, SitDown = 2 }        // reserved 3..=255
// semantic map: Move ↔ ORDER_MOVE(4), Attack ↔ ORDER_ATTACK(5)/ORDER_ATTACK_CLIENT(18),
//               SitDown ↔ ORDER_SITDOWN(17)   (CommonHeader.h:517-535)
enum ChatMode { Normal = 1, Secret = 2, Town = 3, Head = 4, Home = 5, Ask = 6,
                System = 8, Villager = 9, Party = 10, King = 11, NewsItem = 12,
                Guild = 13 }                                // MsgType-System.h:1085-1097
const MAX_CHAT_BYTES: usize = 255;
```

### 8.3 Field layouts and goldens (pre-computed here — the F-07 expectation source)

Tests assert **these literals**; a golden may never be regenerated from the encoder under
test. Client and `goonzu-rs` tests both cite this section.

**A — Order/Move, x=100, y=200** (`kind 00`, `x = 0x0064`, `y = 0x00C8`):

```
payload (5 B): 00 | 64 00 | C8 00
frame:         0A 00 | 00 | F0 00 | 00 64 00 C8 00        (usSize 10, cmd 240 = 0x00F0)
```

**B — Order/Attack, target 12345 (`0x00003039`):**

```
payload (5 B): 01 | 39 30 00 00
frame:         0A 00 | 00 | F0 00 | 01 39 30 00 00
```

**C — Order/SitDown:**

```
payload (1 B): 02
frame:         06 00 | 00 | F0 00 | 02
```

**D — Chat, `ChatMode::Normal (1)`, text `"hi"`:** no `siDataSize`, no NUL (D6).

```
payload (3 B): 01 | 68 69
frame:         08 00 | 00 | 75 00 | 01 68 69              (usSize 8 = 6 + 2)
```

**E — Quit / Front** (empty payloads, D2): `05 00 00 6B 00` · `05 00 00 6D 00`.

**F — Warp, village 3:** frame `07 00 00 92 00 03 00` (cmd 146 = 0x0092).
**G — NPCPOS, kind 5:** frame `07 00 00 56 14 05 00` (cmd 5206 = 0x1456).

**H — `RESPONSE_ORDER`/Move, char 12345, x=100, y=200** (two-stage gate input, D8):

```
payload (9 B): 39 30 00 00 | 00 | 64 00 | C8 00
frame:         0E 00 | 00 | F1 00 | 39 30 00 00 00 64 00 C8 00   (usSize 14, cmd 241 = 0x00F1)
```

**I — `RESPONSE_ORDER`/SitDown:** payload `39 30 00 00 | 02` (5 B), frame
`0A 00 00 F1 00 39 30 00 00 02`.

Refusal fixtures: 241 with `len < 5` → `Truncated { expected: 5 }`; Move with `len 8` →
`Truncated { expected: 9 }`; Move with `len 10` → `TrailingBytes { expected: 9 }`;
`kind = 200` → `UnknownOrderKind { kind: 200 }`; chat `""` → `EmptyChat`; chat 256 B →
`ChatTooLong { len: 256, max: 255 }`.

### 8.4 API shape (`src/game/action.rs`) and server-mirror obligations

```rust
pub const REQUEST_ORDER: u16 = 240;   // + REQUEST_CHAT/QUIT/FRONT/WARP/NPCPOS, RESPONSE_ORDER
pub enum Order { Move { x: u16, y: u16 }, Attack { target_char: u32 }, SitDown }
pub fn encode_order(order: &Order) -> Vec<u8>;                    // 5 / 5 / 1 B
pub async fn send_order(link: &mut GameLink, order: &Order) -> Result<(), GameError>;

pub enum ChatMode { /* §8.2, repr(u8) */ }
pub fn encode_chat(mode: ChatMode, text: &str) -> Result<Vec<u8>, ActionError>; // D6 refusals
pub async fn send_chat(link: &mut GameLink, mode: ChatMode, text: &str)
    -> Result<(), ActionError>;                                   // ActionError::Send(GameError)

pub fn encode_warp(village: u16) -> [u8; 2];
pub fn encode_npc_pos(kind: u16) -> [u8; 2];
pub const EMPTY_BODY: [u8; 0] = [];                               // quit / front
// inbound (dispatcher, additive): parse_response_order →
//   DispatchOutcome::Ordered { char_unique: i32, target: ApplyTarget, order: Order }
```

`send_*` is `link.send(OPCODE, &payload).await` — the existing gate (`entry.rs:530-535`;
pre-handshake = `GameError::NotHandshaken`, zero bytes written). Payload builders are pure
and tested without I/O.

**`goonzu-rs` mirror checklist (the server half of this contract):**

1. Same opcodes (D1), same LE field order and per-kind lengths (§8.2-8.3), same
   `MAX_CHAT_BYTES` and kind/mode tables.
2. Validation duties: map-bounds check on `Move.x/y`; `village < village_count`;
   `kind`/`mode` known-or-refuse; strict UTF-8; **sender identity always from the
   connection, never from the payload** (the outbound `CharUnique` strip, D4, makes this
   structurally impossible to spoof).
3. Its tests assert the §8.3 goldens literally (F-07 on both sides); a drift between the
   client's and server's golden sets is a finding against whoever changed §8 without a
   WO revision.
4. `RESPONSE_ORDER` is emitted for neighbour visibility of intent; **authoritative
   position keeps flowing through opcode 200** (§3.2) — 241 never replaces it.

## 9. Legacy bloated layout vs modern lean payload (comparison)

| Intent | Legacy wire (verified against `GoonZu_Source/`) | B | Modern lean (§8) | B | Cut |
|---|---|---:|---|---:|---|
| **Move** | `cltOrderData`: `siOrder(1) siHostMode(1) pad(2) siPara1(4) siPara2(2) siPara3(2) siPara4(2) pad(2) dwPushedTime(4) CharUnique(4)`; x/y hidden in superword `x \| (y<<16)` (`order.cpp:148`) | 24 | `kind=0, x: u16, y: u16` | 5 | **−79%** |
| **Attack** | same 24-B struct; target buried in `siPara*` | 24 | `kind=1, target_char: u32` | 5 | **−79%** |
| **SitDown** | same 24-B struct | 24 | `kind=2` | 1 | **−96%** |
| **Chat** | `siDataSize(2) siChatMode(2) siSystemTxt(4) + text + NUL`, self-describing length, fixed `TCHAR[256]`, CP949 | 8+len+1 | `mode: u8 + UTF-8 text`, frame-delimited | 1+len | **−8 B** + no NUL + no fixed array |
| **Warp** | `SI32` village index (`FanPriestDlg.h:42`) | 4 | `village: u16` | 2 | **−50%** |
| **Quit / Front** | 0-B body via `cltMsg(type, 0, NULL)` wrapper | 0 | 0-B (frame header only) | 0 | wrapper semantics → spec (D2) |
| **NPCPOS** | `SI16 siKind` struct | 2 | `kind: u16` | 2 | same size, no struct wrapper |
| **241 echo** | full 24-B `cltOrderData` re-sent to neighbours | 24 | `char: u32 + kind: u8 + payload` | 9 (Move) | **−62%** |
| **Per-message overhead** | 4 pad bytes + in-payload `siDataSize` (chat) + `cltMsg` duplication | 6-12 | none — one 5-B frame header, fields only | 0 | all in-payload overhead gone |
| **Fields carried but unused** | `siHostMode` (always `BY_PLAYER`), `siPara3/4`, `dwPushedTime`, outbound `CharUnique`, `siSystemTxt` | 13+ | deleted (D4) | 0 | intent-only payloads |

Semantics preserved unchanged (not "modernized away"): opcode identity (D1), server-side
position authority (§3.2), target resolution rules (`Client-SystemNPC.cpp:546-553`),
`ORDER_*`/`CHATMODE_*` meaning (§8.2), and the 5-byte frame header of the existing codec
(D2).

## 10. Test invariants and the Phase 1/2 outline

Pattern authorities: opcode-const citation tables (`src/game/world_entry/tests.rs:178-189`),
encode→wire→decode round trips (`world_entry/tests.rs:740-860`), shared harness
(`src/game/test_support.rs`: `local_listener`, `frame`, `encode_link`; F-01 loopback-only,
F-07 no-expectation-from-parser), heartbeat send tests (`src/client/heartbeat/tests.rs`).

| # | Invariant | Phase |
|---|---|---|
| T1 | Constant table: opcodes 240/241/117/107/109/146/5206, `OrderKind` 0/1/2, every `ChatMode` value, `MAX_CHAT_BYTES = 255` — each with its §8 citation | 1 |
| T2 | Move golden: `encode_order(Move{100,200})` == §8.3-A's 5 bytes; frame `0A 00 00 F0 00 00 64 00 C8 00` | 1 |
| T3 | Attack/SitDown goldens §8.3-B/C — kind tag first byte, variant-only tails (D3) | 1 |
| T4 | x/y LE round trip for `(0,0)`, `(100,200)`, `(65535,65535)` — pins D5 against word-swap or superword regressions | 1 |
| T5 | Chat golden §8.3-D: payload = `mode ‖ text`, **no** `siDataSize`, **no** NUL; frame `usSize == 6 + text.len()` (D2/D6) | 1 |
| T6 | Chat refusals **by name**: `EmptyChat`, `ChatTooLong { len: 256, max: 255 }` (D6 — never clamp) | 1 |
| T7 | System goldens §8.3-E/F/G: quit/front empty, warp 2 B, npcpos 2 B | 1 |
| T8 | 241 two-stage gate: 9-B Move and 5-B SitDown parse; `len 4` → `Truncated{expected:5}`; Move `len 8` → `Truncated{9}`; Move `len 10` → `TrailingBytes{9}` (D8, by name) | 2 |
| T9 | 241 unknown kind `200` → `UnknownOrderKind { kind: 200 }` **by name**, before any length pass (D3/D8) | 2 |
| T10 | Target resolution triple: self unique → `Local`, table id → `Nearby`, unknown → `Unmatched` (`Client-SystemNPC.cpp:546-553`) | 2 |
| T11 | Report-only: the `WorldChar` table is bit-identical before/after `Ordered` dispatch — no state write; position authority stays 200 (§3.2) | 2 |
| T12 | Regression: all pre-existing tests (337) unchanged and green; `other => Unhandled` (`dispatcher.rs:885`) still holds | 1+2 |
| T13 | Send gate: pre-handshake send → `GameError::NotHandshaken` and **zero bytes written** (`entry.rs:524-533`; cf. `world_entry/tests.rs:924`) | 1 |
| T14 | Loopback round trip: encode → session-cipher wire → peer read-back == goldens for Order/Move and Chat | 1 |

**Phase 1 (outbound)** = `action.rs` + `action/tests.rs`: T1-T7, T13-T14.
**Phase 2 (inbound)** = additive `dispatcher.rs` + additive `dispatcher/tests.rs`: T8-T12.
**Phase 3** = driver wiring — excluded (§6), separate WO.

## 11. Forbidden shortcuts

- Editing anything outside §7 to make a test pass (finding first, P3-6).
- Deriving a golden from the encoder/server under test (F-07) — §8.3's literals rule.
- **Regressing to a legacy shape**: padding bytes, a struct transmute, `siDataSize`,
  a fixed 256 array, NUL terminators, CP949 for new messages, the `x | (y<<16)`
  superword, `BY_*`/`dwPushedTime`/`CharUnique`/`siSystemTxt` fields, or a silent chat
  clamp (D4-D7, §9) — each is a §10 test failure by design.
- Emitting a reserved order kind, or accepting one silently on 241 (D3/T9).
- Writing `WorldChar.x/y` from a 241 (T11) or inventing a position upload (§3.2).
- Implementing 267 (§3.2), migrating shipped opcodes, or changing the cipher/frame
  header (§6/D2).
- Any external network, GPU, window, or display in a gate test; any live-server run
  promoted to a gate (live = evidence only).
- Touching `TASK_MATRIX.md` / `CONTRIBUTING*.md` / `README.md` / `GoonZu_Source/**`.

## 12. Evidence required (Verifier, clean clone at the delivery SHA)

- `evidence/<step>/` standing bundle: `head.txt`, `check.log`, `clippy.log`, `fmt.log`,
  `test.log`, `doc.log`, `env.txt`, `SHA256SUMS`; matrix test number copied from
  `test.log` (baseline 337 + new Phase 1/2 tests).
- **Mutation probes (Adversary equivalent)**, each breaking a named test loudly:
  1. swap Move's x/y words → T2/T4 fail;
  2. re-introduce a `siDataSize` prefix on chat → T5 fails;
  3. make the 241 gate fixed-length (all kinds 9) → T8's SitDown case fails;
  4. shift `OrderKind` values (`Move = 1`) → T1/T3 fail;
  5. resolve the local 241 target as `Unmatched` → T10 fail.
  Probe description, mutated diff, failing output, pristine restore — all recorded.
- Write-guard confirmation: `git diff --stat` limited to §7 files; `GoonZu_Source/**`,
  matrix, contributing, README untouched.
- Mirror confirmation: §8.4 checklist signed off (server-side goldens match §8.3) — or a
  finding raised if `goonzu-rs` has not yet landed the mirror.

## 13. Definition of done

A Step 7 delivery is `PASSED` only when, on a clean clone at the delivery SHA:

1. All five mandatory gates exit-0 at **337 + N** tests (N = new Phase 1/2 tests, from
   `test.log`), `fmt` 0 diffs, `doc` 0 warnings, clean tree.
2. §8.3 goldens A-I pass byte-exactly; T1-T14 all present and green.
3. Every §5 decision (D1-D9) is pinned by a failing-if-corrected test (legacy-shape
   regressions fail §10 by construction).
4. All five §12 mutation probes show their named test failing, with pristine restores.
5. Write guard holds: only §7 files changed; matrix/contributing/README/legacy tree
   untouched (row appended later by Custodian/Scribe, `CONTRIBUTING_PHASE3.md:185`).
6. F-07 holds: every expectation traces to §8.3 or a legacy *semantics* citation — never
   to the encoder/parser under test.
7. §8.4 mirror checklist recorded (or its gap filed as a finding).
8. No external network in any test; no position upload, no 267, no simulation (§6).

## 14. Return blocks (append; do not rewrite the sections above)

### Delivery note - Implementer

_(to be filled at delivery: branch, delivery SHA, gate table with exact counts, per-phase
test inventory, `git diff` write-guard confirmation, any F-7.N findings, mutation-probe
results, mirror-checklist status.)_

### Verdict - Verifier

_(to be filled by the Verifier lane: S4 clean-clone re-run, mutation-probe adjudication,
mirror confirmation, DoD 1-8 check, merge authorization.)_

## 15. Future Invariant & Server Alignment Note (Rev 2.1 - 2026-11-05)

Recorded from the Pre-Implementation QA Audit
(`docs/audit/2026-11-05-step-7-protocol-spec-qa-audit.md`, Verifier `psi-ginger`,
APPROVED at `e32f481`). These four advisories clarify the §8.4 mirror
obligations only: no byte, size, opcode, or golden in §8 changes
(§8.4.3); editing any golden in §8.3 still requires a WO revision.

- **Q-1 - NPCPOS `kind` is opaque and server-defined.** The §8.4.2 duty
  "`kind`/`mode` known-or-refuse" is scoped to `OrderKind` (0/1/2, §8.2/D3)
  and `ChatMode` (§8.2) only. `REQUEST_NPCPOS`'s `kind: u16` carries no
  enumerated valid set in this WO - the legacy `siKind` semantics were never
  extracted (§3.3, inbound 5207 "not extracted") - so it is an
  **opaque `u16`: the client may send any value; the server treats it as
  server-defined (own range check or pass-through) and must not assume a shared
  enum with the client**. Neither endpoint may invent a "known set" the other
  does not hold - that asymmetry is the one desync Q-1 exists to prevent.
- **Q-2 - Server refusals are unobservable by the client.** For map-bounds
  violations, `village >= village_count`, unknown `kind`/`mode`, non-UTF-8 text,
  and chat over `MAX_CHAT_BYTES`: the client has no receive path for any such
  signal (§6 admits no inbound opcode beyond 241), so **any server refusal
  behavior - silent drop, disconnect, or internal error handling - is
  conformant**, and no finding will be raised against the server for choosing
  one. Refusals stay refusals, never truncation, on both encode paths (D6),
  independent of server behavior.
- **Q-3 - The 241 broadcast scope is server-internal.** The client is
  report-only (D8/T11: `DispatchOutcome::Ordered` never writes `WorldChar`;
  position truth stays opcode 200). The legacy emission mechanism
  (`SendNeighbourMsg`, `char-order.cpp:118-119`) is the semantic reference for
  *why* 241 exists, but the actual interest set (radius/cell/all-in-map) is
  **server-defined logic the client does not model, observe, or depend on** -
  no client test may pin it.
- **Q-4 - `char_unique` `u32` (§8.2) vs `i32` (§8.4/code) is notation
  only.** The wire representation is **4 bytes little-endian either way** (the
  repo parses with `i32_at` / `i32::from_le_bytes`, `entry.rs:252`); the two
  types are bit-identical for every valid id, and both endpoints read and write
  the same 4 bytes. No endpoint may treat sign or width as a contract
  difference.

Status: advisory only - Phase 1 may proceed unchanged. Q-1 must remain in force
when the `goonzu-rs` mirror checklist (§8.4) is signed off (DoD item 7);
Q-2 through Q-4 may be folded into any future WO touch.
