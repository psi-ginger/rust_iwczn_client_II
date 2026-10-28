// Transparent TCP relay for live capture: listen -> dial target, pump both
// directions while appending each direction's raw bytes to its own file.
// Usage: relay <listen_host:port> <target_host:port> <c2s_file> <s2c_file>
use std::env;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};

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
    if args.len() != 5 {
        eprintln!("usage: relay <listen> <target> <c2s_file> <s2c_file>");
        std::process::exit(2);
    }
    let listener = TcpListener::bind(&args[1]).expect("bind");
    eprintln!("relay listening {} -> {}", args[1], args[2]);
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
        std::thread::spawn(move || pump(up2, down, c2s));
        std::thread::spawn(move || pump(down2, up, s2c));
    }
}
