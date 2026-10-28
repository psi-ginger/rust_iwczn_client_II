// evidence/live/decode_tool.rs - Verifier's offline read-back tool (2026-10-28).
//
// PROVENANCE: this source was run as a TEMPORARY integration test inside the
// repo working tree (`cargo test --test zz_live_capture_decode -- --nocapture`)
// and deleted immediately after the read-back; the tree was clean afterwards
// (head.txt). It is preserved here purely for reproducibility of decode.log /
// decode2.log. To re-run: copy it to tests/zz_live_capture_decode.rs, adjust
// the two capture paths to wherever cap/ lives, run the command above, delete
// it again. It is NOT part of the suite and must never be committed to tests/.
//
// WHAT IT PROVES: the live server->client game stream decodes 69/69 frames
// integrity-true with the client's OWN inbound cipher (Cipher::link_cipher(),
// entry.rs:399 - key1=0x12 key2=0x34, +1/+1 per call), consuming exactly
// 50,996/50,996 captured bytes. Any framing or cipher assumption wrong would
// break integrity or leave bytes unconsumed.
//
// decode2.log (this exact source): record-grade dump of the refused LOGINMANY
// payload - every one of the 34 record heads with full fields.
// decode.log (an earlier variant of the same walk, additionally printing a
// line per frame): the 69-frame listing.
use goonzu_client::net::cipher::Cipher;

#[test]
fn dump_live_loginmany_records() {
    let raw = std::fs::read(r"C:\Users\DK2\AppData\Local\Temp\opencode\cap\s2c.bin").unwrap();
    let mut cipher = Cipher::link_cipher();
    let mut off = 0usize;
    let mut frames = 0usize;
    let mut loginmany: Option<Vec<u8>> = None;
    while off + 2 <= raw.len() {
        let size = u16::from_le_bytes([raw[off], raw[off + 1]]) as usize;
        if size < 5 || off + size > raw.len() {
            break;
        }
        let wire = &raw[off..off + size];
        let mut dest = vec![0u8; size];
        let ok = cipher.decode(wire, &mut dest).expect("decode");
        assert!(ok, "integrity failed at frame {frames}");
        let cmd = u16::from_le_bytes([dest[3], dest[4]]);
        if cmd == 6 {
            loginmany = Some(dest[5..size].to_vec());
        }
        off += size;
        frames += 1;
    }
    eprintln!("frames={frames} consumed={off}/{}", raw.len());
    let p = loginmany.expect("one LOGINMANY frame");
    let rec = 68usize;
    eprintln!(
        "payload={} divisible by 68: {} ; by 592: {}",
        p.len(),
        p.len() % rec == 0,
        p.len() % 592
    );
    assert_eq!(p.len() % rec, 0);
    let n = p.len() / rec;
    for i in 0..n {
        let b = &p[i * rec..i * rec + rec];
        let w = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        eprintln!(
            "rec[{i:02}] off={:04} w0={:#010x} (uiType={}) id@4={:#010x} ({}) w8={:#010x} w12={:#010x} w16={:#010x}",
            i * rec,
            w(0),
            w(0) & 1,
            w(4),
            w(4),
            w(8),
            w(12),
            w(16)
        );
    }
    // first 80 bytes hex, for the record
    eprintln!("payload[0..80] = {:02x?}", &p[..80]);
}
