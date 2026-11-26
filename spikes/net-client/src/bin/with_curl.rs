//! The GET through the system's `curl`, as a child process.
use std::process::Command;

fn main() {
    let url = std::env::args().nth(1).expect("a URL");
    let out = Command::new("curl")
        .args(["-sS", "--fail", "--max-time", "5", "--compressed", "-A", "talos-spike/0.1", "--", &url])
        .output()
        .expect("curl runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    println!("{} bytes", out.stdout.len());
}
