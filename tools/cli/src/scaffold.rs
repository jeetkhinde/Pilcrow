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
        "created web-backend scaffold at {} (run `pilcrow-cli check-arch` there)",
        root.display()
    );
    Ok(())
}

fn create_scaffold(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("apps/web/src"))?;
    fs::create_dir_all(root.join("apps/backend/src"))?;

    fs::write(
        root.join("Cargo.toml"),
        r#"[workspace]
members = ["apps/web", "apps/backend"]
resolver = "2"
"#,
    )?;

    fs::write(
        root.join("README.md"),
        "# Pilcrow Web-Backend Convention\n\nBrowser -> web (BFF) -> backend APIs.\n",
    )?;

    fs::write(
        root.join("Pilcrow.toml"),
        r#"[web]
host = "127.0.0.1"
port = 3000
backend_url = "http://127.0.0.1:4000"

[backend]
host = "127.0.0.1"
port = 4000
"#,
    )?;

    fs::write(
        root.join("apps/web/Cargo.toml"),
        r#"[package]
name = "web"
version = "0.1.0"
edition = "2024"

[dependencies]
pilcrow-web = "*"
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

    Ok(())
}
