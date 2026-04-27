use std::process::Command;

pub fn handle_dev(_args: &[String]) -> Result<(), String> {
    let has_watch = Command::new("cargo")
        .args(["watch", "--version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if has_watch {
        println!("  pilcrow dev  (cargo-watch — reloads on every save)");
        let status = Command::new("cargo")
            .args(["watch", "-s", "cargo run"])
            .env("PILCROW_DEV", "1")
            .status()
            .map_err(|e| format!("failed to run cargo watch: {e}"))?;
        if !status.success() {
            return Err(format!("cargo watch exited with status: {status}"));
        }
    } else {
        eprintln!("hint: `cargo install cargo-watch` for automatic reloads on save");
        println!("  pilcrow dev  (no file-watching — restart manually to reload)");
        let status = Command::new("cargo")
            .arg("run")
            .env("PILCROW_DEV", "1")
            .status()
            .map_err(|e| format!("failed to run cargo: {e}"))?;
        if !status.success() {
            return Err(format!("cargo run exited with status: {status}"));
        }
    }

    Ok(())
}
