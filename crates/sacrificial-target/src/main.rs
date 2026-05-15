//! Idle binary used as a stand-in injection target by the launcher's tests.
//! Prints its PID to stdout and then parks the main thread forever.

use std::io::Write;

fn main() {
    let pid = std::process::id();
    println!("sacrificial-target ready, pid={pid}");
    std::io::stdout().flush().ok();
    std::thread::park();
}
