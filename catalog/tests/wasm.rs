#![feature(exit_status_error)]

use std::process::Command;

#[test]
fn test() {
    Command::new("wasm-pack")
        .args(&["test", "--chrome", "--headless", "--test", "integrate"])
        .status()
        .unwrap()
        .exit_ok()
        .unwrap();
}
