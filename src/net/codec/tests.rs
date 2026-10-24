use super::fixtures::legacy_send;
use super::*;
use crate::net::cipher::Cipher;
use crate::net::session::Session;
use crate::protocol::header::{WireHeader, MAX_FRAME_SIZE};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::fixtures::legacy::{Case, Divergence, CASES, DIVERGENT};

fn unhex(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "odd-length hex {hex:?}");
    hex.as_bytes()
        .chunks(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("hex is ascii");
            u8::from_str_radix(text, 16).expect("hex digit")
        })
        .collect()
}

/// A well-formed plaintext frame: 5-byte header + payload, reserved byte
/// zeroed — built straight from the Step 1.2 header type so encoder tests
/// have an expectation source that is not the encoder itself.
fn plaintext_frame(cmd: u16, payload: &[u8]) -> Vec<u8> {
    let header = WireHeader::new(cmd, payload.len()).expect("payload fits a frame");
    let mut bytes = header.encode().to_vec();
    bytes.extend_from_slice(payload);
    bytes
}

/// Pulls everything queued; `Err` only for a refused decoder.
fn drain(decoder: &mut Decoder) -> Result<Vec<Vec<u8>>, CodecError> {
    let mut frames = Vec::new();
    while let Some(frame) = decoder.next_frame()? {
        frames.push(frame.into_bytes());
    }
    Ok(frames)
}

/// Replays one recorded case through this decoder, step by step, and
/// compares both answers that matter: was the read refused (`feed` vs the
/// legacy's `Completion` return) and which bytes came out (`next_frame`
/// vs the legacy's `GetFirstPacket` reads). The two recorded divergences
/// are asserted explicitly so neither can drift into agreement by
/// accident. Adapted from `gz-net/tests/legacy_vectors.rs`.
fn replay(case: &Case) -> Result<(), String> {
    let divergence = DIVERGENT
        .iter()
        .find(|(name, _)| *name == case.name)
        .map(|(_, kind)| *kind);
    let mut decoder = Decoder::new();
    let mut sent: Vec<u8> = Vec::new();

    for (index, step) in case.steps.iter().enumerate() {
        let at = format!("{} read {index}", case.name);
        let chunk = unhex(step.chunk);

        if divergence == Some(Divergence::SubHeader) {
            // BUG-013: the legacy handed out a frame with no room for
            // usCmd; this port refuses the connection. Asserted in both
            // directions, pinned in data rather than left to a comment.
            assert!(
                decoder.feed(&chunk).is_err(),
                "{at}: expected this port to refuse"
            );
            assert!(
                step.completed,
                "{at}: legacy is recorded as accepting a sub-header frame"
            );
            assert_eq!(step.packets.len(), 1, "{at}: expected one legacy frame");
            assert!(
                unhex(step.packets[0]).len() < HEADER_SIZE,
                "{at}: divergence case must be a sub-header frame"
            );
            assert!(
                decoder.next_frame().is_err(),
                "{at}: a refused decoder must stay refused"
            );
            return Ok(());
        }

        if divergence == Some(Divergence::RingCapacity) {
            // BUG-014: the legacy's ring wrapped and handed out a
            // different frame than the one sent. Having no ring, this port
            // must accept every read and reassemble what was written.
            decoder
                .feed(&chunk)
                .map_err(|e| format!("{at}: this port refused ({e})"))?;
            sent.extend_from_slice(&chunk);
            continue;
        }

        match decoder.feed(&chunk) {
            Ok(()) if !step.completed => {
                return Err(format!("{at}: the legacy refused but this port accepted"));
            }
            Err(err) if step.completed => {
                return Err(format!(
                    "{at}: this port refused ({err}) but the legacy accepted"
                ));
            }
            Err(_) => {
                // Both refused: a refusal hands out nothing.
                if !step.packets.is_empty() {
                    return Err(format!("{at}: a refused read handed out a frame"));
                }
                assert!(
                    decoder.next_frame().is_err(),
                    "{at}: a refused decoder must stay refused"
                );
                return Ok(());
            }
            Ok(()) => {}
        }

        let got = drain(&mut decoder).map_err(|e| format!("{at}: {e}"))?;
        let want: Vec<Vec<u8>> = step.packets.iter().map(|p| unhex(p)).collect();
        if got != want {
            return Err(format!(
                "{at}: frames differ\n  legacy: {want:x?}\n  this port: {got:x?}"
            ));
        }
    }

    if divergence == Some(Divergence::RingCapacity) {
        let got = drain(&mut decoder).expect("ring case must not be refused");
        assert_eq!(
            got,
            vec![sent.clone()],
            "this port must deliver exactly the frame the peer sent, reassembled"
        );
        let legacy: Vec<Vec<u8>> = case
            .steps
            .iter()
            .flat_map(|step| step.packets)
            .map(|p| unhex(p))
            .collect();
        assert_ne!(
            legacy,
            vec![sent],
            "the legacy no longer mangles this frame; re-check BUG-014"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------- fixture

#[test]
fn fixture_is_intact_and_divergences_are_pinned() {
    // A ground-truth table must describe itself: every recorded packet's
    // length agrees with its own declared usSize, or the copy is corrupt.
    for case in CASES {
        for step in case.steps {
            for packet in step.packets {
                let bytes = unhex(packet);
                let declared = u16::from_le_bytes([bytes[0], bytes[1]]);
                assert_eq!(
                    usize::from(declared),
                    bytes.len(),
                    "{}: packet length disagrees with its own header",
                    case.name
                );
            }
        }
    }

    // Names unique and the fixture is not trivially small.
    let mut names: Vec<&str> = CASES.iter().map(|case| case.name).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len(), "duplicate case names");
    assert!(CASES.len() >= 15, "the framing fixture shrank");

    // Exactly the reviewed divergences: BUG-013's three sub-header sizes
    // and BUG-014's over-ring frame. Nothing else may differ from the
    // legacy silently.
    assert_eq!(
        DIVERGENT,
        &[
            ("declared_2", Divergence::SubHeader),
            ("declared_3", Divergence::SubHeader),
            ("declared_4", Divergence::SubHeader),
            ("ring_smaller_than_frame", Divergence::RingCapacity),
        ],
        "the divergence list changed; re-verify it against the legacy"
    );

    // A refusal is the last read of its case: the legacy caller
    // disconnects and stops feeding, so later reads never happened.
    for case in CASES {
        let last_refusal = case
            .steps
            .iter()
            .rposition(|step| !step.completed)
            .unwrap_or(usize::MAX);
        for (index, step) in case.steps.iter().enumerate() {
            if !step.completed {
                assert_eq!(
                    index, last_refusal,
                    "{}: reads recorded after a refusal",
                    case.name
                );
                assert!(
                    step.packets.is_empty(),
                    "{}: a refused read handed out a frame",
                    case.name
                );
            }
        }
    }

    // Boundaries sit on this port's constants, measured not assumed.
    let by_name = |name: &str| {
        CASES
            .iter()
            .find(|case| case.name == name)
            .unwrap_or_else(|| panic!("missing case {name}"))
    };
    assert!(by_name("declared_5").steps[0].completed);
    assert!(by_name("declared_64992_short_body").steps[0].completed);
    assert!(by_name("declared_64992_short_body").steps[0]
        .packets
        .is_empty());
    assert!(!by_name("declared_64993_over_limit").steps[0].completed);
    assert!(!by_name("declared_65535").steps[0].completed);
    assert_eq!(MAX_FRAME_SIZE, 64992);
}

#[test]
fn decoder_agrees_with_the_compiled_legacy_on_every_recorded_read() {
    for case in CASES {
        if let Err(why) = replay(case) {
            panic!("{why}");
        }
    }
}

// ------------------------------------------------------------ plaintext

#[test]
fn single_frame_is_emitted_byte_exact() {
    let wire = plaintext_frame(30000, b"hello goonzu");
    let mut decoder = Decoder::new();

    decoder.feed(&wire).expect("valid stream");
    let frames = drain(&mut decoder).expect("valid stream");

    assert_eq!(frames, vec![wire.clone()]);
    assert!(decoder.next_frame().unwrap().is_none(), "queue drained");
    assert!(decoder.is_empty());
    assert_eq!(decoder.failure(), None);
    assert!(decoder.cipher().is_none(), "front-link style: no cipher");
}

#[test]
fn a_frame_split_mid_header_and_mid_payload_reassembles() {
    let wire = plaintext_frame(0x0102, &[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    let mut decoder = Decoder::new();

    // 1 byte: the first size byte alone — no header yet.
    decoder.feed(&wire[..1]).expect("valid");
    assert!(decoder.next_frame().unwrap().is_none());
    assert_eq!(decoder.buffered(), 1);

    // Through byte 3: usSize complete (guard already ran), ucChk in hand,
    // but usCmd is cut mid-field and no body has arrived.
    decoder.feed(&wire[1..4]).expect("valid");
    assert!(decoder.next_frame().unwrap().is_none());
    assert_eq!(decoder.buffered(), 4);

    // Header complete, half the payload.
    decoder.feed(&wire[4..8]).expect("valid");
    assert!(decoder.next_frame().unwrap().is_none());

    // The rest completes it.
    decoder.feed(&wire[8..]).expect("valid");
    let frames = drain(&mut decoder).expect("valid");
    assert_eq!(frames, vec![wire]);
    assert!(decoder.is_empty());
}

#[test]
fn byte_by_byte_delivery_reassembles() {
    let wire = plaintext_frame(7, &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
    let mut decoder = Decoder::new();

    let mut emitted = Vec::new();
    for (index, byte) in wire.iter().enumerate() {
        decoder.feed(&[*byte]).expect("valid");
        while let Some(frame) = decoder.next_frame().expect("valid") {
            assert_eq!(index, wire.len() - 1, "emitted before the last byte");
            emitted.push(frame.into_bytes());
        }
    }
    assert_eq!(emitted, vec![wire]);
}

#[test]
fn multi_frame_bursts_drain_in_order() {
    let expected: Vec<Vec<u8>> = (0u16..5)
        .map(|i| {
            let payload = vec![i as u8; i as usize + 1];
            plaintext_frame(100 + i, &payload)
        })
        .collect();
    let mut burst: Vec<u8> = Vec::new();
    for frame in &expected {
        burst.extend_from_slice(frame);
    }

    // One burst, one feed: every frame must come out, in order.
    let mut decoder = Decoder::new();
    decoder.feed(&burst).expect("valid burst");
    assert_eq!(drain(&mut decoder).expect("valid"), expected);
    assert!(decoder.is_empty());

    // The same frames split across feeds at frame boundaries and inside
    // frames alike, drained between feeds. Frames end at 6, 13, 21, 30,
    // 40 of the 40-byte burst, so the cuts land inside frames, exactly on
    // one boundary, and at the end.
    let mut decoder = Decoder::new();
    let mut got = Vec::new();
    let cuts = [0, 7, 20, 21, 33, burst.len()];
    for window in cuts.windows(2) {
        decoder.feed(&burst[window[0]..window[1]]).expect("valid");
        got.extend(drain(&mut decoder).expect("valid"));
    }
    assert_eq!(got, expected, "chunk boundaries must not change framing");
}

#[test]
fn a_legal_size_with_an_incomplete_body_is_held_not_refused() {
    // 64992 declared (inside the guard) with only part of the body
    // present: a slow peer must not be disconnected — the refusal
    // decision comes from the header, the wait from the body.
    let mut chunk = vec![0u8; 45];
    chunk[..2].copy_from_slice(&MAX_FRAME_SIZE.to_le_bytes());

    let mut decoder = Decoder::new();
    decoder.feed(&chunk).expect("valid, just incomplete");
    assert!(decoder.failure().is_none());
    assert!(decoder.next_frame().unwrap().is_none());
    assert_eq!(decoder.buffered(), 45);

    // Complete it: exactly MAX_FRAME_SIZE bytes on the wire, fully
    // delivered — the one boundary the recorded fixture cannot carry
    // (130 KB of hex), so it is checked end to end here.
    let body = vec![0x5A; MAX_FRAME_SIZE as usize - 45];
    decoder.feed(&body).expect("valid");
    assert_eq!(
        decoder.buffered(),
        MAX_FRAME_SIZE as usize,
        "45 buffered + body must equal the declared size exactly"
    );
    let frames = drain(&mut decoder).expect("valid");
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].len(), MAX_FRAME_SIZE as usize);
    assert!(decoder.is_empty(), "nothing may trail a full-size frame");
    // And its own header agrees, through the Step 1.2 decoder.
    let header = WireHeader::decode(&frames[0]).expect("validated frame");
    assert_eq!(header.size(), MAX_FRAME_SIZE);
    assert_eq!(header.payload_len(), MAX_FRAME_SIZE as usize - HEADER_SIZE);
}

#[test]
fn the_guard_table_refuses_each_bad_size_at_feed_time_and_sticks() {
    // (size bytes, expected refusal): the legacy's own <2 guard, BUG-013's
    // sub-header range, and the receive ceiling — all three refusal points
    // of ITZRecvBuffer.cpp:163,172 plus Step 1.2's deliberate divergence.
    let cases: [(u16, CodecError); 7] = [
        (
            0,
            CodecError::Header(HeaderError::SizeBelowMinimum { declared: 0 }),
        ),
        (
            1,
            CodecError::Header(HeaderError::SizeBelowMinimum { declared: 1 }),
        ),
        (
            2,
            CodecError::Header(HeaderError::SizeBelowHeader { declared: 2 }),
        ),
        (
            3,
            CodecError::Header(HeaderError::SizeBelowHeader { declared: 3 }),
        ),
        (
            4,
            CodecError::Header(HeaderError::SizeBelowHeader { declared: 4 }),
        ),
        (
            64993,
            CodecError::Header(HeaderError::SizeTooLarge { declared: 64993 }),
        ),
        (
            u16::MAX,
            CodecError::Header(HeaderError::SizeTooLarge { declared: u16::MAX }),
        ),
    ];

    for (declared, expected) in cases {
        let mut chunk = vec![0xFF; 16];
        chunk[..2].copy_from_slice(&declared.to_le_bytes());

        let mut decoder = Decoder::new();
        assert_eq!(decoder.feed(&chunk), Err(expected), "declared {declared}");
        // Sticky: every later call keeps the original refusal, and clear()
        // (which the legacy only ever uses to reset a live session,
        // ITZSession.cpp:181,305) does not resurrect it.
        assert_eq!(decoder.failure(), Some(expected));
        assert_eq!(decoder.feed(&[0x05, 0x00]), Err(expected));
        assert_eq!(decoder.next_frame(), Err(expected));
        decoder.clear();
        assert_eq!(decoder.next_frame(), Err(expected));
    }

    // ...while 5 (the smallest frame that describes itself) and 64992
    // (the ceiling) are accepted. Feed exact-length chunks: trailing zero
    // bytes after a *complete* frame would otherwise parse as the next
    // header's size 0 — a genuinely illegal header a real stream would
    // rightly be refused for.
    let mut header_only = vec![0u8; HEADER_SIZE];
    header_only[..2].copy_from_slice(&5u16.to_le_bytes());
    let mut decoder = Decoder::new();
    assert_eq!(decoder.feed(&header_only), Ok(()));
    assert_eq!(decoder.failure(), None);
    let frame = decoder
        .next_frame()
        .expect("valid")
        .expect("a complete 5-byte frame comes out whole");
    assert_eq!(frame.size(), 5);

    // An in-limit size with an incomplete body is held, not refused.
    let mut at_ceiling = vec![0u8; 8];
    at_ceiling[..2].copy_from_slice(&MAX_FRAME_SIZE.to_le_bytes());
    let mut decoder = Decoder::new();
    assert_eq!(
        decoder.feed(&at_ceiling),
        Ok(()),
        "64992 sits exactly on the receive limit"
    );
    assert!(decoder.next_frame().unwrap().is_none(), "body incomplete");
}

#[test]
fn a_good_frame_ahead_of_a_bad_header_is_never_delivered() {
    // The recorded `good_then_over_limit` case, spelled out: the legacy's
    // Completion refuses the whole read without committing the good frame
    // in front of it, so this must too. This is why headers are validated
    // eagerly in feed() rather than at pull time.
    let mut chunk = plaintext_frame(9, b"deliver me");
    chunk.extend_from_slice(&[0xE1, 0xFD, 0x00, 0x00, 0x00]); // 64993

    let mut decoder = Decoder::new();
    assert_eq!(
        decoder.feed(&chunk),
        Err(CodecError::Header(HeaderError::SizeTooLarge {
            declared: 64993
        }))
    );
    assert_eq!(
        decoder.next_frame(),
        Err(CodecError::Header(HeaderError::SizeTooLarge {
            declared: 64993
        })),
        "the queued good frame must die with the connection, not escape"
    );
}

#[test]
fn clear_discards_bytes_but_keeps_a_refusal() {
    let wire = plaintext_frame(3, b"partial");

    // Healthy decoder: clear() resets the buffer like ClearBuffer does.
    let mut decoder = Decoder::new();
    decoder.feed(&wire[..4]).expect("valid");
    assert_eq!(decoder.buffered(), 4);
    assert!(!decoder.is_empty());
    decoder.clear();
    assert!(decoder.is_empty());

    // And the decoder is usable again after a clear...
    decoder.feed(&wire).expect("valid");
    assert_eq!(
        drain(&mut decoder).expect("valid"),
        vec![wire.clone()],
        "clear() on a healthy decoder must not lose later frames"
    );

    // ...but a refused one stays refused: the legacy only ever cleared
    // buffers to reset a live session (ITZSession.cpp:181,305), never to
    // rescue one that was refused.
    let mut refused = Decoder::new();
    assert!(refused.feed(&[0x00, 0x00, 0x00]).is_err());
    refused.clear();
    assert!(
        refused.next_frame().is_err(),
        "clear() must not resurrect a refusal"
    );
    assert!(refused.failure().is_some());
}

#[test]
fn compression_flag_and_reserved_byte_pass_through_the_decoder() {
    // usCmd bit 15 is the LZF flag the legacy clears only *after*
    // decompressing (GameProtocol.cpp:141): framing must carry it.
    let wire = [0x05, 0x00, 0x00, 0x05, 0x80];
    let mut decoder = Decoder::new();
    decoder.feed(&wire).expect("valid");
    let frame = decoder.next_frame().expect("valid").expect("one frame");
    assert!(frame.is_compressed());
    assert_eq!(frame.cmd(), 0x8005, "the flag must not be masked");
    assert_eq!(frame.cmd_id(), 5);
    assert!(frame.payload().is_empty());

    // The reserved byte is not a checksum: on a plaintext link (the front
    // link) it arrives arbitrary and must ride through untouched.
    let wire = [0x05, 0x00, 0xA5, 0x01, 0x00];
    let mut decoder = Decoder::new();
    decoder.feed(&wire).expect("valid");
    let frame = decoder.next_frame().expect("valid").expect("one frame");
    assert_eq!(frame.integrity(), 0xA5);
    assert_eq!(frame.as_bytes(), wire, "plaintext bytes pass verbatim");
}

// ---------------------------------------------------------------- cipher

/// Four plaintext frames of assorted sizes/commands for cipher tests.
fn cipher_frames() -> Vec<Vec<u8>> {
    vec![
        plaintext_frame(5, b""),
        plaintext_frame(0x000A, &[1, 2, 3]),
        plaintext_frame(30000, &[0xDE, 0xAD, 0xBE, 0xEF]),
        plaintext_frame(0x0042, &(0u8..64).collect::<Vec<u8>>()),
    ]
}

#[test]
fn ciphered_frames_reassemble_across_irregular_splits_and_bursts() {
    let frames = cipher_frames();

    // The peer encodes each frame with its outbound link cipher...
    let mut tx = Cipher::link_cipher();
    let mut wire: Vec<u8> = Vec::new();
    for frame in &frames {
        wire.extend_from_slice(&tx.encode(frame).expect("encode"));
    }

    // ...and we decode with the paired inbound instance, feeding the
    // stream in deliberately ragged chunks (1,4,2,7,3,11,5,... bytes) so
    // every boundary lands somewhere different: mid-size-field, mid-cipher
    // body, on frame edges, and inside bursts.
    let mut decoder = Decoder::with_cipher(Cipher::link_cipher());
    let steps = [1usize, 4, 2, 7, 3, 11, 5];
    let mut got: Vec<Vec<u8>> = Vec::new();
    let mut at = 0usize;
    let mut round = 0usize;
    while at < wire.len() {
        let step = steps[round % steps.len()].min(wire.len() - at);
        round += 1;
        decoder.feed(&wire[at..at + step]).expect("valid stream");
        at += step;
        got.extend(drain(&mut decoder).expect("valid stream"));
    }

    assert_eq!(got, frames, "deciphered frames must be byte-exact");
    assert!(decoder.is_empty());

    // Peer counters ended in step: one cipher call per frame on each side
    // (Step 1.3's invariant, exercised through the framing layer).
    let rx = decoder.cipher().expect("cipher active");
    assert_eq!((rx.key1(), rx.key2()), (tx.key1(), tx.key2()));
    assert_eq!(rx.key1(), 0x12u8.wrapping_add(frames.len() as u8));
    assert_eq!(rx.key2(), 0x34u8.wrapping_add(frames.len() as u8));
}

#[test]
fn cipher_keys_advance_exactly_once_per_pulled_frame() {
    let frames = cipher_frames();
    let mut tx = Cipher::link_cipher();
    let mut wire: Vec<Vec<u8>> = frames
        .iter()
        .map(|frame| tx.encode(frame).expect("encode"))
        .collect();

    let mut decoder = Decoder::with_cipher(Cipher::link_cipher());
    let start = (0x12u8, 0x34u8);

    // Feed frame 1, do not pull: no consumption, no tick (the legacy only
    // calls Decode at GetFirstPacket time).
    decoder.feed(&wire[0]).expect("valid");
    assert_eq!(
        (
            decoder.cipher().unwrap().key1(),
            decoder.cipher().unwrap().key2()
        ),
        start,
        "feed alone must not advance the keys"
    );

    // Pull it: exactly one tick.
    assert!(decoder.next_frame().unwrap().is_some());
    assert_eq!(
        (
            decoder.cipher().unwrap().key1(),
            decoder.cipher().unwrap().key2()
        ),
        (start.0.wrapping_add(1), start.1.wrapping_add(1))
    );

    // Pull the remaining three: three more ticks, ending in step with the
    // transmitting peer.
    wire.remove(0);
    let mut rest: Vec<u8> = Vec::new();
    for chunk in &wire {
        rest.extend_from_slice(chunk);
    }
    decoder.feed(&rest).expect("valid");
    assert_eq!(drain(&mut decoder).expect("valid"), frames[1..].to_vec());

    let rx = decoder.cipher().unwrap();
    assert_eq!((rx.key1(), rx.key2()), (tx.key1(), tx.key2()));
    assert_eq!(
        (rx.key1(), rx.key2()),
        (
            start.0.wrapping_add(frames.len() as u8),
            start.1.wrapping_add(frames.len() as u8)
        ),
        "N pulled frames = N ticks, no more, no less"
    );
}

#[test]
fn a_tampered_frame_fails_integrity_and_reports_the_legacy_log_fields() {
    let plaintext = plaintext_frame(0x1234, &[0x11, 0x22, 0x33]);
    let mut tx = Cipher::link_cipher();
    let mut wire = tx.encode(&plaintext).expect("encode");

    // Corrupt one payload byte — the recorded cipher vectors' corrupt-
    // payload case (E2). Integrity must catch it.
    let last = wire.len() - 1;
    wire[last] ^= 0xFF;

    let mut decoder = Decoder::with_cipher(Cipher::link_cipher());
    decoder
        .feed(&wire)
        .expect("framing is happy: usSize is clear");
    let err = decoder.next_frame().expect_err("integrity must fail");

    let (size, cmd, key1, key2) = match err {
        CodecError::Integrity {
            size,
            cmd,
            key1,
            key2,
        } => (size, cmd, key1, key2),
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert_eq!(size as usize, wire.len(), "usSize logged from the header");
    // The legacy logged usCmd straight off the raw (ciphertext) buffer —
    // Client-Winsock.cpp:878 reads it before any decode succeeds.
    assert_eq!(cmd, u16::from_le_bytes([wire[3], wire[4]]));
    // Post-tick position: Decode advanced both keys on entry
    // (ITZEncrypt.cpp:72-73) before refusing.
    assert_eq!((key1, key2), (0x13, 0x35));

    // Sticky, like the legacy's `return false` teardown: the keys ticked,
    // so there is no way to continue.
    assert_eq!(decoder.failure(), Some(err));
    assert_eq!(decoder.feed(&wire), Err(err));
    assert_eq!(decoder.next_frame(), Err(err));
}

#[test]
fn a_ciphered_decoder_refuses_a_plaintext_stream() {
    // Wrong bytes for the key in force: the integrity byte disagrees, so a
    // link wired up with the wrong cipher state dies instead of quietly
    // handing up garbage.
    let wire = plaintext_frame(0x1234, b"hi");
    let mut decoder = Decoder::with_cipher(Cipher::link_cipher());
    decoder.feed(&wire).expect("framing only checks usSize");

    let err = decoder.next_frame().expect_err("must not accept");
    let frame_len = wire.len();
    assert!(
        matches!(err, CodecError::Integrity { size, .. } if usize::from(size) == frame_len),
        "expected an integrity failure on this {frame_len}-byte frame, got {err:?}"
    );
    assert!(decoder.failure().is_some(), "the link must stay dead");
}

#[test]
fn a_plaintext_decoder_never_touches_its_bytes_or_keys() {
    // Front-link parity: inbound is plaintext (Step 3.1), so decoding is
    // a pure framing pass — no cipher call, no integrity risk.
    let wire = plaintext_frame(30000, b"secret key incoming");
    let mut decoder = Decoder::new();
    decoder.feed(&wire).expect("valid");
    let frame = decoder.next_frame().expect("valid").expect("one");
    assert_eq!(frame.as_bytes(), &wire);
    assert!(decoder.cipher().is_none());
}

// ------------------------------------------------------- session transport

#[tokio::test]
async fn decoder_reassembles_a_dribbled_stream_read_off_the_session() {
    // Composition proof for Steps 2.1 + 2.2: frames written in four
    // dribbles over a real socket. TCP may coalesce or split them
    // arbitrarily on any given run — the decoder must reassemble the same
    // bytes every time either way.
    let frame_a = plaintext_frame(30000, b"front handshake");
    let frame_b = plaintext_frame(5, &[9, 8, 7, 6]);
    let mut wire = frame_a.clone();
    wire.extend_from_slice(&frame_b);

    let (listener, addr) = {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        (listener, addr)
    };
    let writer = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.expect("accept");
        let cuts = [0, 3, 9, wire.len() - 4, wire.len()];
        for window in cuts.windows(2) {
            stream
                .write_all(&wire[window[0]..window[1]])
                .await
                .expect("write");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        drop(stream);
    });

    let mut session = Session::connect(addr).await.expect("connect");
    let mut decoder = Decoder::new();
    let mut got: Vec<Vec<u8>> = Vec::new();

    loop {
        let mut buf = [0u8; 64];
        let read = session.read(&mut buf).await.expect("read");
        if read == 0 {
            break; // writer closed: EOF
        }
        decoder.feed(&buf[..read]).expect("valid stream");
        while let Some(frame) = decoder.next_frame().expect("valid stream") {
            got.push(frame.into_bytes());
        }
        if got.len() == 2 {
            break;
        }
    }
    writer.await.expect("writer task");

    assert_eq!(got, vec![frame_a, frame_b]);
    assert!(decoder.failure().is_none());
    assert!(decoder.is_empty());
}

// ------------------------------------------------------- encoder captures

#[test]
fn encoder_reproduces_every_recorded_cxx_capture_byte_for_byte() {
    for case in legacy_send::CASES {
        // One encoder per case, starting at the capture's initial keys —
        // every op then plays out as the next frame on the same link.
        let mut encoder = Encoder::with_cipher(Cipher::session_cipher(case.key1, case.key2));
        for (index, op) in case.ops.iter().enumerate() {
            let at = format!("{} op {index}", case.name);
            let input = unhex(op.input);
            let declared = usize::from(u16::from_le_bytes([input[0], input[1]]));
            assert!(
                declared >= HEADER_SIZE,
                "{at}: capture is a frame too small for any encoder ({declared} bytes)"
            );
            assert!(
                declared <= input.len(),
                "{at}: capture declares {declared} bytes but carries only {}",
                input.len()
            );
            // Bytes past usSize are not part of the frame — the legacy
            // reads exactly usSize (C8 pins that case by name).
            let frame = &input[..declared];
            let cmd = u16::from_le_bytes([frame[3], frame[4]]);
            let payload = &frame[5..];

            let got = encoder
                .encode(cmd, payload)
                .unwrap_or_else(|e| panic!("{at}: encoder refused a recorded frame: {e}"));
            let want = unhex(op.output);
            assert_eq!(
                got.as_bytes(),
                want.as_slice(),
                "{at}: wire bytes differ\n  recorded C++: {want:x?}\n  this port:    {:x?}\n  input frame:  {frame:x?}",
                got.as_bytes()
            );
        }
    }
    assert_eq!(
        legacy_send::CASES.len(),
        11,
        "the capture table changed — re-verify it against gz-cipher's vectors.rs"
    );
}

#[test]
fn encoder_rebuilds_every_recorded_plaintext_packet() {
    // Every frame `GetFirstPacket` handed out across the recorded
    // ITZRecvBuffer captures, rebuilt from its (cmd, payload) by the
    // plaintext encoder: header prepend byte-exact, reserved byte zeroed.
    let mut checked = 0usize;
    let mut skipped = 0usize;
    for case in CASES {
        for step in case.steps {
            for packet in step.packets {
                let bytes = unhex(packet);
                if bytes.len() < HEADER_SIZE {
                    // Only the BUG-013 divergent cases record sub-header
                    // packets; an encoder must not be able to build one.
                    assert!(
                        DIVERGENT.iter().any(|(name, kind)| {
                            *name == case.name && *kind == Divergence::SubHeader
                        }),
                        "{}: a non-BUG-013 case recorded a sub-header packet",
                        case.name
                    );
                    skipped += 1;
                    continue;
                }
                assert_eq!(
                    usize::from(u16::from_le_bytes([bytes[0], bytes[1]])),
                    bytes.len(),
                    "{}: recorded packet's usSize must equal its length",
                    case.name
                );
                let cmd = u16::from_le_bytes([bytes[3], bytes[4]]);
                let got = Encoder::new()
                    .encode(cmd, &bytes[5..])
                    .unwrap_or_else(|e| panic!("{}: recorded frame refused: {e}", case.name));
                if bytes[2] == 0 {
                    assert_eq!(
                        got.as_bytes(),
                        bytes.as_slice(),
                        "{}: rebuild must be byte-identical",
                        case.name
                    );
                } else {
                    // The nonzero reserved byte in these fixtures was
                    // chosen by the recording harness — no legacy
                    // producer ever writes ucChk (Step 1.2; the field is
                    // only ever read, `GameProtocol.cpp:82,124`), and
                    // the encoder emits 0 the way gz-net's `Frame::new`
                    // does. Everything else must still match exactly.
                    assert_eq!(got.as_bytes()[0..2], bytes[0..2], "{}: usSize", case.name);
                    assert_eq!(got.as_bytes()[2], 0, "{}: reserved byte zeroed", case.name);
                    assert_eq!(
                        got.as_bytes()[3..],
                        bytes[3..],
                        "{}: cmd + payload",
                        case.name
                    );
                }
                checked += 1;
            }
        }
    }
    assert_eq!(
        skipped, 3,
        "exactly the three BUG-013 packets are unbuildable"
    );
    assert!(
        checked >= 11,
        "expected the recorded packet set, got {checked}"
    );
}

#[test]
fn set_cipher_switches_the_outbound_side_mid_connection() {
    let mut encoder = Encoder::new();
    assert!(encoder.cipher().is_none(), "starts unset");
    let before = encoder.encode(5, b"before the key").unwrap();
    assert_eq!(
        before.as_bytes(),
        plaintext_frame(5, b"before the key"),
        "an unset link emits header + payload verbatim"
    );

    // Opcode 30000 hands over the seeds; Steps 3.1/4.2 wire this call.
    encoder.set_cipher(Cipher::session_cipher(18, 52));
    // Two frames with identical content, replayed from the recorded C3
    // capture pair: after the transition exactly one key tick per frame.
    let first = encoder.encode(0xCCBB, &[0xDD, 0xEE, 0xFF]).unwrap();
    assert_eq!(first.as_bytes(), unhex("080012dd5475f02f"));
    let second = encoder.encode(0xCCBB, &[0xDD, 0xEE, 0xFF]).unwrap();
    assert_eq!(second.as_bytes(), unhex("080013e5547bea37"));

    let cipher = encoder.cipher().expect("cipher active after set_cipher");
    assert_eq!(
        (cipher.key1(), cipher.key2()),
        (20, 54),
        "two frames after set_cipher = two ticks"
    );
    assert_eq!(before.wire_len(), 5 + b"before the key".len());
}

#[test]
fn encoded_frames_round_trip_through_the_decoder_in_both_modes() {
    let messages: &[(u16, &[u8])] = &[
        (1, b"unique please"),
        (5, &[9, 8, 7, 6]),
        (30000, b""),
        // Compression flag set: framing must pass it through untouched
        // (the caller owns compression, both directions).
        (0x8005, b"flag survives framing"),
    ];
    let expected: Vec<Vec<u8>> = messages
        .iter()
        .map(|(cmd, payload)| plaintext_frame(*cmd, payload))
        .collect();
    let wire: Vec<u8> = expected.concat();

    // Plaintext link.
    let mut plain_encoder = Encoder::new();
    let mut plain_decoder = Decoder::new();
    let built: Vec<Vec<u8>> = messages
        .iter()
        .map(|(cmd, payload)| {
            plain_encoder
                .encode(*cmd, payload)
                .expect("fits the guard")
                .into_bytes()
        })
        .collect();
    assert_eq!(built, expected, "encoder == the Step 1.2 frame builder");
    plain_decoder.feed(&wire).expect("valid stream");
    assert_eq!(
        drain(&mut plain_decoder).expect("valid stream"),
        expected,
        "plaintext round trip is byte-exact"
    );

    // Ciphered link: the client's Encode and the server's Decode share
    // one key stream (Step 1.3's invariant), so both sides start from
    // identical initial keys.
    let mut encoder = Encoder::with_cipher(Cipher::session_cipher(18, 52));
    let mut decoder = Decoder::with_cipher(Cipher::session_cipher(18, 52));
    let mut ciphered = Vec::new();
    for (cmd, payload) in messages {
        ciphered.extend_from_slice(encoder.encode(*cmd, payload).unwrap().as_bytes());
    }
    assert_ne!(ciphered, wire, "the cipher must actually encipher");
    decoder.feed(&ciphered).expect("valid stream");
    let mut frames: Vec<Frame> = Vec::new();
    while let Some(frame) = decoder.next_frame().expect("valid stream") {
        frames.push(frame);
    }
    let frame_bytes: Vec<Vec<u8>> = frames.iter().map(|f| f.as_bytes().to_vec()).collect();
    assert_eq!(
        frame_bytes, expected,
        "ciphered round trip is byte-exact, reserved byte included"
    );
    assert_eq!(frames[2].cmd(), 30000, "empty-payload frame survives");
    assert_eq!(frames[3].cmd(), 0x8005, "encoder must not mask the flag");

    // N encodes and N decodes ticked N times each: the pair is still in
    // step, so the next frame continues the same keystream.
    let enc_keys = (
        encoder.cipher().expect("cipher").key1(),
        encoder.cipher().expect("cipher").key2(),
    );
    let dec_keys = (
        decoder.cipher().expect("cipher").key1(),
        decoder.cipher().expect("cipher").key2(),
    );
    assert_eq!(enc_keys, (18 + 4, 52 + 4), "one tick per encoded frame");
    assert_eq!(enc_keys, dec_keys, "sender and receiver keystreams agree");
}

#[test]
fn an_oversized_payload_is_refused_before_the_cipher_ticks() {
    let max_payload = MAX_FRAME_SIZE as usize - HEADER_SIZE;
    let mut encoder = Encoder::with_cipher(Cipher::session_cipher(18, 52));

    let largest = encoder
        .encode(7, &vec![0xAB; max_payload])
        .expect("the ceiling itself fits");
    assert_eq!(largest.wire_len(), MAX_FRAME_SIZE as usize);
    assert_eq!(largest.size(), MAX_FRAME_SIZE);
    assert_eq!(
        encoder.cipher().expect("cipher").key1(),
        19,
        "the accepted frame ticked exactly once"
    );

    assert_eq!(
        encoder.encode(7, &vec![0xAB; max_payload + 1]),
        Err(CodecError::Header(HeaderError::PayloadTooLarge {
            len: max_payload + 1,
        })),
    );
    assert_eq!(
        encoder.cipher().expect("cipher").key1(),
        19,
        "a refused encode must leave the keys untouched (documented divergence)"
    );

    // What we emit at the ceiling is exactly what the peer's guard lets
    // through: the receive side accepts its own worst case.
    let mut decoder = Decoder::new();
    decoder
        .feed(largest.as_bytes())
        .expect("our own ceiling frame is legal");
    assert_eq!(drain(&mut decoder).unwrap(), vec![largest.into_bytes()]);
}

// ------------------------------------------------------- encoder flushing

#[tokio::test]
async fn send_flushes_exact_wire_bytes_over_the_session() {
    // Composition proof for Steps 2.1 + 1.2 + 1.3 + 2.3: two ciphered
    // frames flushed through a real socket. The expectation is built
    // from the Step 1.2 header and the Step 1.3 cipher directly — each
    // already vector-verified on its own — never from the encoder under
    // test.
    let messages: &[(u16, &[u8])] = &[(30000, b"front handshake"), (5, &[9, 8, 7, 6])];
    let mut expectation = Cipher::session_cipher(18, 52);
    let expected: Vec<u8> = messages
        .iter()
        .flat_map(|(cmd, payload)| {
            expectation
                .encode(&plaintext_frame(*cmd, payload))
                .expect("cipher accepts a well-formed frame")
        })
        .collect();
    let want_len = expected.len();
    let mut encoder = Encoder::with_cipher(Cipher::session_cipher(18, 52));

    let (listener, addr) = {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        (listener, addr)
    };
    let reader = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.expect("accept");
        let mut got = Vec::with_capacity(want_len);
        let mut buf = [0u8; 7]; // small buffer: TCP may deliver in any split
        while got.len() < want_len {
            let read = stream.read(&mut buf).await.expect("read");
            if read == 0 {
                break; // writer closed before the frame arrived
            }
            got.extend_from_slice(&buf[..read]);
        }
        got
    });

    let mut session = Session::connect(addr).await.expect("connect");
    encoder
        .send(&mut session, messages[0].0, messages[0].1)
        .await
        .expect("first flush");
    encoder
        .send(&mut session, messages[1].0, messages[1].1)
        .await
        .expect("second flush");
    drop(session);

    let got = reader.await.expect("reader task");
    assert_eq!(
        got.as_slice(),
        expected.as_slice(),
        "wire bytes must equal header + cipher composition"
    );

    let cipher = encoder.cipher().expect("cipher still active");
    assert_eq!(
        (cipher.key1(), cipher.key2()),
        (20, 54),
        "two sends = two ticks"
    );
}
