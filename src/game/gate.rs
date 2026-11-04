//! Shared frame gate for the game Server parsers (Step 6's C4): opcode
//! first, then the exact (or declared) length — never a field read first.
//! One core, two thin wrappers (`dispatcher::gate_frame`,
//! `world_entry::gate_frame`) so every existing call site stays
//! byte-identical ("share core, not blind dedup", Step 7.0 audit line 33).

/// Why `gate_frame` refused a frame — variant and field names identical to
/// both caller error enums (`DispatchError`, `WorldEntryError`), so each
/// wrapper maps it one-for-one into its own type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GateViolation {
    /// The frame's opcode is not the one this parser accepts.
    UnexpectedOpcode {
        /// The parser's expected opcode.
        expected: u16,
        /// The opcode actually seen on the frame.
        cmd: u16,
    },
    /// The body is shorter than the declared length.
    Truncated {
        /// The opcode being parsed — equal to `expected` on every path past
        /// the opcode guard (disclosed in the Phase 4 evidence).
        opcode: u16,
        /// The declared body length.
        expected: usize,
        /// The actual body length.
        got: usize,
    },
    /// The body is longer than the declared length.
    TrailingBytes {
        /// The opcode being parsed — equal to `expected` on every path past
        /// the opcode guard (disclosed in the Phase 4 evidence).
        opcode: u16,
        /// The declared body length.
        expected: usize,
        /// The actual body length.
        got: usize,
    },
}

/// Gate: opcode, then the exact (or declared) length — never a field read
/// first. The two donor bodies were equal apart from style (dispatcher's
/// early-return `if`s vs world_entry's `match`); `opcode` carries `cmd`,
/// which equals `expected_cmd` on every path past the opcode guard.
///
/// # Errors
///
/// `GateViolation::UnexpectedOpcode` first, then `GateViolation::Truncated` /
/// `GateViolation::TrailingBytes` for any length that disagrees with
/// `expected_len`.
pub(crate) fn gate_frame(
    cmd: u16,
    expected_cmd: u16,
    expected_len: usize,
    payload: &[u8],
) -> Result<(), GateViolation> {
    if cmd != expected_cmd {
        return Err(GateViolation::UnexpectedOpcode {
            expected: expected_cmd,
            cmd,
        });
    }
    if payload.len() < expected_len {
        return Err(GateViolation::Truncated {
            opcode: expected_cmd,
            expected: expected_len,
            got: payload.len(),
        });
    }
    if payload.len() > expected_len {
        return Err(GateViolation::TrailingBytes {
            opcode: expected_cmd,
            expected: expected_len,
            got: payload.len(),
        });
    }
    Ok(())
}
