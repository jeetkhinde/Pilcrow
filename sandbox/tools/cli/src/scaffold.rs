use std::fs;
use std::path::{Path, PathBuf};

pub fn handle_new(args: &[String]) -> Result<(), String> {
    if args.len() != 3 || args[0] != "--convention" || args[1] != "web-backend" {
        return Err("expected: new --convention web-backend <dir>".to_string());
    }

    let root = PathBuf::from(&args[2]);
    if root.exists() {
        return Err(format!("destination already exists: {}", root.display()));
    }

    create_scaffold(&root).map_err(|err| err.to_string())?;
    println!(
        "created web-backend scaffold at {} (run `cargo run -p pilcrow-cli -- check-arch` there)",
        root.display()
    );
    Ok(())
}

fn create_scaffold(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("apps/web/src"))?;
    fs::create_dir_all(root.join("apps/backend/src"))?;
    fs::create_dir_all(root.join("crates/contracts/src"))?;

    fs::write(
        root.join("Cargo.toml"),
        r#"[workspace]
members = ["apps/web", "apps/backend", "crates/contracts"]
resolver = "2"
"#,
    )?;

    fs::write(
        root.join("README.md"),
        "# Pilcrow Web-Backend Convention\n\nBrowser -> web (BFF) -> backend APIs.\n",
    )?;

    fs::write(
        root.join("apps/web/Cargo.toml"),
        r#"[package]
name = "web"
version = "0.1.0"
edition = "2024"

[dependencies]
pilcrow-web = "*"
pilcrow-api-client-rest = "*"
"#,
    )?;

    fs::write(
        root.join("apps/web/src/main.rs"),
        "fn main() { println!(\"web app scaffold\"); }\n",
    )?;

    fs::write(
        root.join("apps/backend/Cargo.toml"),
        r#"[package]
name = "backend"
version = "0.1.0"
edition = "2024"

[dependencies]
pilcrow-core = "*"
"#,
    )?;

    fs::write(
        root.join("apps/backend/src/main.rs"),
        "fn main() { println!(\"backend app scaffold\"); }\n",
    )?;

    fs::write(
        root.join("crates/contracts/Cargo.toml"),
        r#"[package]
name = "contracts"
version = "0.1.0"
edition = "2024"
"#,
    )?;

    fs::write(root.join("crates/contracts/src/lib.rs"), "\n")?;

    Ok(())
}
