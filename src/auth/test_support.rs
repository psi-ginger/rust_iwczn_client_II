// Step 3 (T-3) — shared test support for the auth suites.
//
// The wire constants and frame helpers below were duplicated byte-for-byte
// across the front, world_list and login test suites. They now live here
// exactly once, with their citation docs moved **verbatim** — no citation
// lost, no expectation re-derived from a parser under test (F-07).

use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

use crate::net::cipher::Cipher;

/// `FRONTMSG_RESPONSE_SECRETKEY` as it sits on the wire, in the clear:
/// usSize 10 (`FrontMsg.h:132-141`), ucChk 0 (what the server port writes;
/// the legacy leaves it uninitialized — either parses identically), usCmd
/// 30000 LE, then `ucTemp1 ucTemp2 ucKey1 ucTemp3 ucKey2` with seeds
/// 0xA7/0x5C.
pub(crate) const SECRETKEY_FRAME: [u8; 10] =
    [0x0A, 0x00, 0x00, 0x30, 0x75, 0x00, 0x00, 0xA7, 0x00, 0x5C];

/// The two seeds `SECRETKEY_FRAME` carries, at payload offsets 2 and 4.
pub(crate) const SEED_KEY1: u8 = 0xA7;
pub(crate) const SEED_KEY2: u8 = 0x5C;

/// Builds raw plaintext frame bytes: `[usSize LE][ucChk][usCmd LE][payload]`.
/// Server → client is plaintext on this link, so this is a literal wire
/// image (`front.rs` module docs, asymmetry table).
pub(crate) fn frame(cmd: u16, payload: &[u8]) -> Vec<u8> {
    let size = u16::try_from(5 + payload.len()).expect("frame fits u16");
    let mut bytes = Vec::with_capacity(5 + payload.len());
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes.push(0x00); // ucChk
    bytes.extend_from_slice(&cmd.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

/// Reads one ciphered frame the way the front service does: usSize in the
/// clear selects the length, then the session cipher decodes it. Returns
/// `(wire bytes, plaintext frame)`.
pub(crate) async fn read_ciphered_frame(
    stream: &mut TcpStream,
    cipher: &mut Cipher,
) -> (Vec<u8>, Vec<u8>) {
    let mut head = [0u8; 2];
    stream.read_exact(&mut head).await.unwrap();
    let size = usize::from(u16::from_le_bytes(head));
    let mut raw = vec![0u8; size];
    raw[..2].copy_from_slice(&head);
    stream.read_exact(&mut raw[2..]).await.unwrap();
    let mut plain = vec![0u8; size];
    let accepted = cipher.decode(&raw, &mut plain).expect("decode");
    assert!(
        accepted,
        "the seeded session cipher must satisfy the integrity check"
    );
    (raw, plain)
}
