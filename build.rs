use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../web");

    // Grab the current git revision of the project (useful for logging)
    let version = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_else(|| "Unknown".to_string());

    println!("cargo:rustc-env=GIT_HASH={}", version);
}
