// ctrlc_send - sacrificial Ctrl+C broadcaster for the live-green acceptance
// run (LIVE-01 fix verification, 2026-10-28).
//
// Why this exists: sending CTRL_C from PowerShell proved unreliable (the
// sender died mid-script and/or the event never reached the target). This
// helper does exactly the documented mechanism and nothing else:
//   FreeConsole -> AttachConsole(target) -> SetConsoleCtrlHandler(NULL, TRUE)
//   -> GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0)  [broadcast on that console]
// It may itself be killed by its own event (the ignore flag is best-effort
// across console-host implementations) - that is fine: the event is already
// on the wire to every process sharing the target's console.
//
// Usage: ctrlc_send <pid-of-any-process-on-the-target-console>
use std::env;
use std::thread;
use std::time::Duration;

#[link(name = "kernel32")]
extern "system" {
    fn FreeConsole() -> i32;
    fn AttachConsole(process_id: u32) -> i32;
    fn SetConsoleCtrlHandler(
        handler: Option<extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
    fn GenerateConsoleCtrlEvent(ctrl_event: u32, process_group_id: u32) -> i32;
    fn GetConsoleProcessList(list: *mut u32, size: u32) -> u32;
    fn GetLastError() -> u32;
}

fn main() {
    let pid: u32 = match env::args().nth(1).and_then(|a| a.parse().ok()) {
        Some(pid) => pid,
        None => {
            eprintln!("usage: ctrlc_send <pid>");
            std::process::exit(2);
        }
    };
    unsafe {
        FreeConsole();
        if AttachConsole(pid) == 0 {
            eprintln!("AttachConsole({pid}) failed: winerror {}", GetLastError());
            std::process::exit(3);
        }
        let mut list = [0u32; 64];
        let n = GetConsoleProcessList(list.as_mut_ptr(), 64);
        let pids: Vec<String> = list[..n.min(64) as usize]
            .iter()
            .filter(|p| **p != 0)
            .map(ToString::to_string)
            .collect();
        eprintln!("console of pid {pid}: {n} process(es): {}", pids.join(","));
        SetConsoleCtrlHandler(None, 1);
        if GenerateConsoleCtrlEvent(0, 0) == 0 {
            eprintln!("GenerateConsoleCtrlEvent(CTRL_C, 0) failed: winerror {}", GetLastError());
            std::process::exit(4);
        }
    }
    eprintln!("CTRL_C broadcast delivered to the console of pid {pid}");
    // give the console host time to dispatch to every attached handler
    // before this process detaches by exiting
    thread::sleep(Duration::from_secs(2));
}
