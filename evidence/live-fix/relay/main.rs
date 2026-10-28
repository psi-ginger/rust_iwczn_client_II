// Transparent TCP relay for live capture: listen -> dial target, pump both
// directions while appending each direction's raw bytes to its own file.
// Usage: relay <listen_host:port> <target_host:port> <c2s_file> <s2c_file> [fin_after_secs]
//
// With a 5th argument the relay performs a GRACEFUL close after that many
// seconds: shutdown(Write) on both directions (a clean FIN each way) and
// then stays alive until the peers close, so no unread bytes turn the final
// close into an RST. Used by the LIVE-01 fix acceptance run to exercise the
// driver's clean-server-close teardown path.
use std::env;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::time::Duration;

fn pump(mut src: TcpStream, mut dst: TcpStream, path: String) {
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .expect("open capture file");
    let mut buf = [0u8; 8192];
    loop {
        match src.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let _ = log.write_all(&buf[..n]);
                let _ = log.flush();
                if dst.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let _ = dst.shutdown(Shutdown::Write);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 && args.len() != 6 {
        eprintln!("usage: relay <listen> <target> <c2s_file> <s2c_file> [fin_after_secs]");
        std::process::exit(2);
    }
    let fin_after: Option<u64> = args.get(5).and_then(|s| s.parse().ok());
    let listener = TcpListener::bind(&args[1]).expect("bind");
    eprintln!("relay listening {} -> {} (fin_after={:?})", args[1], args[2], fin_after);
    for incoming in listener.incoming() {
        let up = match incoming {
            Ok(s) => s,
            Err(_) => continue,
        };
        let down = match TcpStream::connect(args[2].as_str()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("downstream dial failed: {e}");
                continue;
            }
        };
        let up2 = up.try_clone().expect("clone up");
        let down2 = down.try_clone().expect("clone down");
        let c2s = args[3].clone();
        let s2c = args[4].clone();
        if let Some(secs) = fin_after {
            // graceful-close watchdog: FIN both directions after `secs`,
            // then stay alive so the pumps drain instead of RST-ing.
            let upf = up.try_clone().expect("clone up for fin");
            let downf = down.try_clone().expect("clone down for fin");
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(secs));
                let _ = upf.shutdown(Shutdown::Write); // FIN to the server side
                let _ = downf.shutdown(Shutdown::Write); // FIN to the client side
                eprintln!("relay: graceful FIN sent in both directions");
                std::thread::sleep(Duration::from_secs(30));
                std::process::exit(0);
            });
        }
        std::thread::spawn(move || pump(up2, down, c2s));
        std::thread::spawn(move || pump(down2, up, s2c));
    }
}
