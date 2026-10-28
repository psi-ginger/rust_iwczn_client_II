// ctrl_probe - decisive test for whether THIS environment delivers console
// ctrl events at all (LIVE-01 fix verification, 2026-10-28).
//
// Launched exactly like the driver (cmd /c with stdio redirection from a
// GUI-less parent => its own new console). Registers a RAW kernel32 console
// ctrl handler that records every event into a file, then waits forever.
// ctrlc_send then broadcasts on the probe's console.
//   - file says FIRED     => OS-level delivery works here; the suspect
//                            becomes the driver's tokio ctrl_c registration.
//   - file stays empty    => this environment's console host does not
//                            dispatch ctrl events (evidence limitation).
// Usage: ctrl_probe <out-file>
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::thread;
use std::time::Duration;

static mut SINK: Option<String> = None;

#[link(name = "kernel32")]
extern "system" {
    fn SetConsoleCtrlHandler(
        handler: Option<extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
}

extern "system" fn on_ctrl(event: u32) -> i32 {
    unsafe {
        if let Some(path) = &SINK {
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "FIRED ctrl_event={event}");
                let _ = f.flush();
            }
        }
    }
    1 // TRUE: handled
}

fn main() {
    let path = env::args().nth(1).expect("usage: ctrl_probe <out-file>");
    unsafe { SINK = Some(path.clone()) };
    let ok = unsafe { SetConsoleCtrlHandler(Some(on_ctrl), 1) };
    let mut f = OpenOptions::new().create(true).append(true).open(&path).unwrap();
    let _ = writeln!(f, "probe up: raw handler registered ok={ok}, pid={}", std::process::id());
    let _ = f.flush();
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
