use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();

    if let Ok(output) = output
        && output.status.success()
    {
        let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !commit.is_empty() {
            println!("cargo:rustc-env=TATRS_GIT_COMMIT={commit}");
        }
    }
}
