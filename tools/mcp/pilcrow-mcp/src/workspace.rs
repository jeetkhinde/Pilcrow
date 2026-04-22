use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FragmentGroup {
    pub dir: String,
    pub url: String,
    pub files: Vec<FileInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutDirStatus {
    pub path: Option<String>,
    pub generated_app: bool,
    pub files: Vec<FileInfo>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectContext {
    pub project_root: String,
    pub manifest_path: String,
    pub app_root: String,
    pub pilcrow_toml: Option<toml::Value>,
    pub crate_versions: BTreeMap<String, String>,
    pub routes: Vec<FileInfo>,
    pub layouts: Vec<FileInfo>,
    pub loading_skeletons: Vec<FileInfo>,
    pub not_found_pages: Vec<FileInfo>,
    pub ui_components: Vec<FileInfo>,
    pub fragments: Vec<FragmentGroup>,
    pub api_routes: Vec<FileInfo>,
    pub params: Vec<FileInfo>,
    pub middleware: Option<FileInfo>,
    pub generated_out_dir: OutDirStatus,
}

#[derive(Debug, Clone)]
pub struct ResolvedProject {
    pub project_root: PathBuf,
    pub manifest_path: PathBuf,
    pub app_root: PathBuf,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PilcrowConfig {
    #[serde(default)]
    fragments: Vec<FragmentConfig>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct FragmentConfig {
    pub dir: String,
    pub url: Option<String>,
}

impl FragmentConfig {
    pub fn url_prefix(&self) -> String {
        self.url.clone().unwrap_or_else(|| {
            self.dir
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or(&self.dir)
                .to_string()
        })
    }
}

pub fn resolve_project(
    current_root: &Path,
    project_root: Option<&str>,
    manifest_path: Option<&str>,
) -> Result<ResolvedProject> {
    let root = project_root
        .map(|path| resolve_against(current_root, path))
        .unwrap_or_else(|| current_root.to_path_buf());
    let manifest = manifest_path
        .map(|path| resolve_against(&root, path))
        .unwrap_or_else(|| {
            let sandbox = root.join("sandbox/apps/web/Cargo.toml");
            if sandbox.exists() {
                sandbox
            } else {
                root.join("Cargo.toml")
            }
        });

    if !manifest.exists() {
        bail!("manifest not found: {}", manifest.display());
    }
    let app_root = manifest
        .parent()
        .context("manifest path has no parent directory")?
        .to_path_buf();

    Ok(ResolvedProject {
        project_root: root,
        manifest_path: manifest,
        app_root,
    })
}

pub fn scan_project(
    current_root: &Path,
    project_root: Option<&str>,
    manifest_path: Option<&str>,
) -> Result<ProjectContext> {
    let resolved = resolve_project(current_root, project_root, manifest_path)?;
    let src = resolved.app_root.join("src");
    let pages = src.join("pages");

    let pilcrow_toml_path = resolved.app_root.join("Pilcrow.toml");
    let pilcrow_source = fs::read_to_string(&pilcrow_toml_path).ok();
    let pilcrow_toml = pilcrow_source
        .as_deref()
        .and_then(|source| toml::from_str::<toml::Value>(source).ok());
    let pilcrow_config = pilcrow_source
        .as_deref()
        .and_then(|source| toml::from_str::<PilcrowConfig>(source).ok())
        .unwrap_or(PilcrowConfig { fragments: vec![] });

    let fragments = pilcrow_config
        .fragments
        .iter()
        .map(|entry| {
            let dir = src.join(&entry.dir);
            Ok(FragmentGroup {
                dir: normalize_path(&entry.dir),
                url: entry.url_prefix(),
                files: list_matching(&dir, |path| has_ext(path, "html"))?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let middleware_path = src.join("middleware.rs");
    let middleware = if middleware_path.exists() {
        Some(file_info(&resolved.app_root, &middleware_path)?)
    } else {
        None
    };

    Ok(ProjectContext {
        project_root: display_path(&resolved.project_root),
        manifest_path: display_path(&resolved.manifest_path),
        app_root: display_path(&resolved.app_root),
        pilcrow_toml,
        crate_versions: crate_versions(&resolved.project_root, &resolved.manifest_path)?,
        routes: list_matching(&pages, |path| {
            has_ext(path, "html")
                && !is_special_page(path, "_layout")
                && !is_special_page(path, "_loading")
                && !is_special_page(path, "_not_found")
        })?,
        layouts: list_matching(&pages, |path| is_special_page(path, "_layout"))?,
        loading_skeletons: list_matching(&pages, |path| is_special_page(path, "_loading"))?,
        not_found_pages: list_matching(&pages, |path| {
            is_special_page(path, "_not_found") || is_special_page(path, "not-found")
        })?,
        ui_components: list_matching(&src.join("ui"), |path| has_ext(path, "html"))?,
        fragments,
        api_routes: list_matching(&src.join("api"), |path| has_ext(path, "rs"))?,
        params: list_matching(&src.join("params"), |path| has_ext(path, "rs"))?,
        middleware,
        generated_out_dir: out_dir_status(&resolved.manifest_path)?,
    })
}

pub fn find_out_dir(manifest_path: &Path) -> Result<PathBuf> {
    let manifest_dir = manifest_path
        .parent()
        .context("manifest path has no parent directory")?;
    let target_dir = find_target_dir(manifest_dir);
    let build_dir = target_dir.join("debug/build");
    if !build_dir.exists() {
        bail!("build directory not found: {}", build_dir.display());
    }

    let mut candidates = Vec::new();
    for entry in fs::read_dir(&build_dir)? {
        let entry = entry?;
        let out_dir = entry.path().join("out");
        let marker = out_dir.join("generated_app.rs");
        if marker.exists() {
            let mtime = marker
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            candidates.push((out_dir, mtime));
        }
    }
    candidates.sort_by_key(|(_, mtime)| *mtime);
    candidates
        .pop()
        .map(|(path, _)| path)
        .context("no generated_app.rs found; run codegen_build first")
}

pub fn list_files_recursive(dir: &Path) -> Result<Vec<FileInfo>> {
    list_matching(dir, |_| true)
}

pub fn ensure_contained(root: &Path, path: &Path) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;
    let parent = path.parent().unwrap_or(root.as_path());
    let canonical_parent = if parent.exists() {
        parent.canonicalize()?
    } else {
        existing_ancestor(parent).canonicalize()?
    };
    if !canonical_parent.starts_with(&root) {
        bail!("path escapes project root: {}", path.display());
    }
    Ok(())
}

pub fn resolve_against(root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn out_dir_status(manifest_path: &Path) -> Result<OutDirStatus> {
    match find_out_dir(manifest_path) {
        Ok(out_dir) => Ok(OutDirStatus {
            path: Some(display_path(&out_dir)),
            generated_app: out_dir.join("generated_app.rs").exists(),
            files: list_files_recursive(&out_dir)?,
            message: "generated artifacts found".to_string(),
        }),
        Err(error) => Ok(OutDirStatus {
            path: None,
            generated_app: false,
            files: vec![],
            message: error.to_string(),
        }),
    }
}

fn crate_versions(project_root: &Path, app_manifest: &Path) -> Result<BTreeMap<String, String>> {
    let mut versions = BTreeMap::new();
    for manifest in [
        app_manifest.to_path_buf(),
        project_root.join("crates/web/Cargo.toml"),
        project_root.join("crates/routekit/Cargo.toml"),
        project_root.join("crates/runtime/Cargo.toml"),
        project_root.join("crates/core/Cargo.toml"),
        project_root.join("crates/client/Cargo.toml"),
        project_root.join("crates/macros/Cargo.toml"),
    ] {
        if let Ok(source) = fs::read_to_string(&manifest) {
            if let Ok(value) = toml::from_str::<toml::Value>(&source) {
                if let Some(package) = value.get("package") {
                    let name = package.get("name").and_then(toml::Value::as_str);
                    let version = package.get("version").and_then(toml::Value::as_str);
                    if let (Some(name), Some(version)) = (name, version) {
                        versions.insert(name.to_string(), version.to_string());
                    }
                }
            }
        }
    }
    Ok(versions)
}

fn find_target_dir(manifest_dir: &Path) -> PathBuf {
    let mut dir = manifest_dir.to_path_buf();
    loop {
        let cargo_config = dir.join(".cargo/config.toml");
        if let Ok(source) = fs::read_to_string(&cargo_config) {
            if let Ok(value) = toml::from_str::<toml::Value>(&source) {
                if let Some(target_dir) = value
                    .get("build")
                    .and_then(|build| build.get("target-dir"))
                    .and_then(toml::Value::as_str)
                {
                    return dir.join(target_dir);
                }
            }
        }
        let manifest = dir.join("Cargo.toml");
        if let Ok(source) = fs::read_to_string(&manifest) {
            if source.contains("[workspace]") {
                return dir.join("target");
            }
        }
        if !dir.pop() {
            return manifest_dir.join("target");
        }
    }
}

fn list_matching(dir: &Path, predicate: impl Fn(&Path) -> bool + Copy) -> Result<Vec<FileInfo>> {
    let mut files = Vec::new();
    if !dir.exists() {
        return Ok(files);
    }
    collect_matching(dir, dir, predicate, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn collect_matching(
    base: &Path,
    dir: &Path,
    predicate: impl Fn(&Path) -> bool + Copy,
    files: &mut Vec<FileInfo>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            collect_matching(base, &path, predicate, files)?;
        } else if predicate(&path) {
            files.push(FileInfo {
                path: path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
                size: metadata.len(),
            });
        }
    }
    Ok(())
}

fn file_info(base: &Path, path: &Path) -> Result<FileInfo> {
    let metadata = path.metadata()?;
    Ok(FileInfo {
        path: path
            .strip_prefix(base)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned(),
        size: metadata.len(),
    })
}

fn has_ext(path: &Path, ext: &str) -> bool {
    path.extension().and_then(|value| value.to_str()) == Some(ext)
}

fn is_special_page(path: &Path, stem: &str) -> bool {
    path.file_stem().and_then(|value| value.to_str()) == Some(stem)
}

fn normalize_path(path: &str) -> String {
    path.trim_start_matches("src/")
        .trim_start_matches('/')
        .to_string()
}

fn existing_ancestor(path: &Path) -> &Path {
    let mut current = path;
    while !current.exists() {
        current = current.parent().unwrap_or_else(|| Path::new("/"));
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_scan_finds_sandbox_routes_and_versions() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let context = scan_project(&root, None, None).unwrap();
        assert!(context.crate_versions.contains_key("pilcrow-web"));
        assert!(context
            .routes
            .iter()
            .any(|route| route.path == "index.html"));
    }

    #[test]
    fn containment_rejects_parent_escape() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let outside = root.join("../outside.txt");
        assert!(ensure_contained(root, &outside).is_err());
    }
}
