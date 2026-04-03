use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_usage();
        std::process::exit(1);
    }

    match args.remove(0).as_str() {
        "check-arch" => {
            if let Err(err) = check_arch(&env::current_dir().expect("read current dir")) {
                eprintln!("architecture check failed: {err}");
                std::process::exit(1);
            }
            println!("architecture check passed");
        }
        "new" => {
            if let Err(err) = handle_new(&args) {
                eprintln!("scaffold failed: {err}");
                std::process::exit(1);
            }
        }
        _ => {
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  pilcrow-cli check-arch");
    eprintln!("  pilcrow-cli new --convention web-backend <dir>");
}

fn handle_new(args: &[String]) -> Result<(), String> {
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

fn check_arch(root: &Path) -> Result<(), String> {
    let workspace_root = find_workspace_root(root)
        .ok_or_else(|| format!("could not locate workspace root from {}", root.display()))?;

    let web_toml = read_file(workspace_root.join("apps/web/Cargo.toml"))?;
    let backend_toml = read_file(workspace_root.join("apps/backend/Cargo.toml"))?;

    let mut failures = Vec::new();

    if !web_toml.contains("pilcrow-web") {
        failures.push("apps/web must depend on `pilcrow-web`".to_string());
    }
    if !web_toml.contains("pilcrow-api-client-rest") {
        failures.push("apps/web must depend on `pilcrow-api-client-rest`".to_string());
    }
    if !backend_toml.contains("pilcrow-core") {
        failures.push("apps/backend must depend on `pilcrow-core`".to_string());
    }

    if web_toml.contains("apps/backend") || web_toml.contains("pilcrow-backend") {
        failures.push("apps/web cannot depend on backend app crate".to_string());
    }
    if backend_toml.contains("apps/web") || backend_toml.contains("pilcrow-web-app") {
        failures.push("apps/backend cannot depend on web app crate".to_string());
    }
    if backend_toml.contains("pilcrow-web") {
        failures.push("apps/backend cannot depend on `pilcrow-web`".to_string());
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

fn read_file(path: PathBuf) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        let candidate = dir.join("Cargo.toml");
        if let Ok(contents) = fs::read_to_string(&candidate)
            && contents.contains("[workspace]")
        {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}
