use std::fs;
use std::path::{Path, PathBuf};

pub fn check_arch(root: &Path) -> Result<(), String> {
    let workspace_root = find_workspace_root(root)
        .ok_or_else(|| format!("could not locate workspace root from {}", root.display()))?;

    let web_toml = read_file(workspace_root.join("apps/web/Cargo.toml"))?;
    let backend_toml = read_file(workspace_root.join("apps/backend/Cargo.toml"))?;

    let mut failures = Vec::new();

    if !web_toml.contains("pilcrow-web") {
        failures.push("apps/web must depend on `pilcrow-web`".to_string());
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

    if web_toml.contains("path = \"../../../crates/") {
        failures.push(
            "apps/web must not use direct framework path dependencies; use version deps + [patch.crates-io]"
                .to_string(),
        );
    }
    if backend_toml.contains("path = \"../../../crates/") {
        failures.push(
            "apps/backend must not use direct framework path dependencies; use version deps + [patch.crates-io]"
                .to_string(),
        );
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
