use std::process::Command;

fn main() {
    // Get git commit hash
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok();

    let git_hash = output
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Check if working directory is dirty
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    let version_suffix = if dirty {
        format!("{git_hash}-dirty")
    } else {
        git_hash
    };

    // Get miden-client version from Cargo.lock
    let miden_client_version = std::fs::read_to_string("../Cargo.lock")
        .ok()
        .and_then(|lock| {
            lock.split("\n[[package]]")
                .find(|block| block.contains("\nname = \"miden-client\""))
                .and_then(|block| {
                    block
                        .lines()
                        .find(|line| line.starts_with("version = "))
                        .map(|line| {
                            line.trim_start_matches("version = ")
                                .trim_matches('"')
                                .to_string()
                        })
                })
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_HASH={version_suffix}");
    println!("cargo:rustc-env=MIDEN_CLIENT_VERSION={miden_client_version}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
    println!("cargo:rerun-if-changed=Cargo.lock");
}
