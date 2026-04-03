use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::Route;

/// One discovered API route source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApiRoute {
    /// URL pattern, e.g. `"/api/todos"` or `"/api/users/:id"`.
    pub pattern: String,
    /// Informational Rust module path, e.g. `"api::todos"`.
    pub module_path: String,
    /// Generated symbol prefix, e.g. `"api_todos"`.
    pub symbol: String,
}

/// Discovered `.html` sources using Pilcrow's Astro-like folder convention.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveredHtmlFiles {
    pub pages: Vec<PathBuf>,
    pub components: Vec<PathBuf>,
    pub layouts: Vec<PathBuf>,
}

/// Discover all `.html` files from `src/pages`, `src/components`, and `src/layouts`.
///
/// `src_root` should point to the project `src` directory.
pub fn discover_html_files(src_root: impl AsRef<Path>) -> io::Result<DiscoveredHtmlFiles> {
    let src_root = src_root.as_ref();

    let mut discovered = DiscoveredHtmlFiles {
        pages: collect_html_files(&src_root.join("pages"))?,
        components: collect_html_files(&src_root.join("components"))?,
        layouts: collect_html_files(&src_root.join("layouts"))?,
    };

    discovered.pages.sort();
    discovered.components.sort();
    discovered.layouts.sort();

    Ok(discovered)
}

/// Build `Route` entries from discovered page files in `src/pages`.
pub fn build_page_routes(src_root: impl AsRef<Path>) -> io::Result<Vec<Route>> {
    let src_root = src_root.as_ref();
    let pages_dir = src_root.join("pages");
    let mut page_files = collect_html_files(&pages_dir)?;
    page_files.sort();

    let pages_dir_text = path_to_unix_slashes(&pages_dir);
    let mut routes = page_files
        .into_iter()
        .map(|path| {
            let file_path = path_to_unix_slashes(&path);
            Route::from_path(&file_path, &pages_dir_text)
        })
        .collect::<Vec<_>>();

    routes.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.pattern.cmp(&b.pattern))
            .then_with(|| a.template_path.cmp(&b.template_path))
    });

    Ok(routes)
}

/// Discover `.rs` route files from `src/api/` (excludes `mod.rs`).
///
/// `src_root` should point to the project `src` directory.
#[allow(dead_code)]
pub(crate) fn discover_api_files(src_root: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
    let mut files = collect_rs_files(&src_root.as_ref().join("api"))?;
    files.sort();
    Ok(files)
}

/// Build `ApiRoute` entries from `.rs` files discovered in `src/api/`.
pub(crate) fn build_api_routes(src_root: impl AsRef<Path>) -> io::Result<Vec<ApiRoute>> {
    let src_root = src_root.as_ref();
    let api_dir = src_root.join("api");
    let api_dir_text = path_to_unix_slashes(&api_dir);

    let mut files = collect_rs_files(&api_dir)?;
    files.sort();

    let mut routes = files
        .into_iter()
        .map(|path| {
            let file_path = path_to_unix_slashes(&path);
            let relative = file_path
                .strip_prefix(&api_dir_text)
                .unwrap_or(&file_path)
                .trim_start_matches('/')
                .to_owned();
            let without_ext = relative
                .strip_suffix(".rs")
                .unwrap_or(&relative)
                .to_owned();
            let (seg_pattern, ..) = crate::route::parse_pattern(&without_ext);
            let api_pattern = if seg_pattern == "/" {
                "/api".to_string()
            } else {
                format!("/api{seg_pattern}")
            };
            ApiRoute {
                pattern: api_pattern,
                module_path: build_module_path(&without_ext),
                symbol: build_api_symbol(&without_ext),
            }
        })
        .collect::<Vec<_>>();

    routes.sort_by(|a, b| a.pattern.cmp(&b.pattern));
    Ok(routes)
}

fn collect_rs_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    walk_rs_dir(root)
}

/// Recursively collect `.rs` route files; excludes `mod.rs`.
fn walk_rs_dir(dir: &Path) -> io::Result<Vec<PathBuf>> {
    fs::read_dir(dir)?.try_fold(Vec::new(), |mut acc, entry| {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            acc.extend(walk_rs_dir(&path)?);
        } else if file_type.is_file() && is_rs_route(&path) {
            acc.push(path);
        }
        Ok(acc)
    })
}

fn is_rs_route(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("rs"))
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n != "mod.rs")
}

/// Build an informational Rust module path from a relative path without extension.
///
/// `users/[id]` → `"api::users::id"`
fn build_module_path(without_ext: &str) -> String {
    let parts = without_ext
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.trim_start_matches('[')
                .trim_end_matches(']')
                .trim_end_matches('?')
                .trim_start_matches("...")
        })
        .collect::<Vec<_>>();

    if parts.is_empty() {
        "api".to_string()
    } else {
        format!("api::{}", parts.join("::"))
    }
}

/// Build the `api_*` symbol name for a file path without extension.
///
/// `users/[id]` → `"api_users_id"`
fn build_api_symbol(without_ext: &str) -> String {
    let (normalized, _) = without_ext.chars().fold(
        (String::new(), false),
        |(mut s, prev_under), ch| {
            let mapped = if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            };
            if mapped == '_' {
                if !prev_under {
                    s.push('_');
                }
                (s, true)
            } else {
                s.push(mapped);
                (s, false)
            }
        },
    );

    let trimmed = normalized.trim_matches('_');
    let base = if trimmed.is_empty() { "index" } else { trimmed };
    format!("api_{base}")
}

fn collect_html_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    walk_dir(root, &mut files)?;
    Ok(files)
}

fn walk_dir(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            walk_dir(&path, files)?;
        } else if file_type.is_file() && is_html(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn is_html(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("html"))
}

fn path_to_unix_slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn discover_html_files_collects_by_folder_kind() {
        let root = mk_temp_root("discover_html");
        let src = root.join("src");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(&src.join("pages/posts/[id].html"), "<h1>Post</h1>");
        write_file(&src.join("components/Card.html"), "<div>Card</div>");
        write_file(&src.join("layouts/Main.html"), "<main>{% block %}</main>");
        write_file(&src.join("pages/ignore.txt"), "ignored");

        let discovered = discover_html_files(&src).expect("expected discovery to succeed");

        assert_eq!(discovered.pages.len(), 2);
        assert_eq!(discovered.components.len(), 1);
        assert_eq!(discovered.layouts.len(), 1);

        cleanup(&root);
    }

    #[test]
    fn build_page_routes_maps_html_paths_to_patterns() {
        let root = mk_temp_root("build_routes");
        let src = root.join("src");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(&src.join("pages/about.html"), "<h1>About</h1>");
        write_file(&src.join("pages/posts/[id].html"), "<h1>Post</h1>");

        let routes = build_page_routes(&src).expect("expected route manifest");
        let patterns = routes
            .iter()
            .map(|r| r.pattern.as_str())
            .collect::<Vec<_>>();

        assert!(patterns.contains(&"/"));
        assert!(patterns.contains(&"/about"));
        assert!(patterns.contains(&"/posts/:id"));

        cleanup(&root);
    }

    #[test]
    fn build_page_routes_returns_empty_when_pages_missing() {
        let root = mk_temp_root("missing_pages");
        let src = root.join("src");
        fs::create_dir_all(&src).expect("create src");

        let routes = build_page_routes(&src).expect("expected empty route list");
        assert!(routes.is_empty());

        cleanup(&root);
    }

    fn mk_temp_root(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pilcrow_routekit_{}_{}_{}",
            prefix,
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, contents).expect("write file");
    }

    fn cleanup(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("cleanup temp dir");
        }
    }

    #[test]
    fn discover_api_files_collects_rs_files() {
        let root = mk_temp_root("discover_api");
        let src = root.join("src");

        write_file(&src.join("api/todos.rs"), "pub fn router() {}");
        write_file(&src.join("api/users/[id].rs"), "pub fn router() {}");
        write_file(&src.join("api/mod.rs"), "// ignored");
        write_file(&src.join("api/users/ignore.txt"), "ignored");

        let files = discover_api_files(&src).expect("discovery should succeed");
        assert_eq!(files.len(), 2, "mod.rs and .txt should be excluded");

        cleanup(&root);
    }

    #[test]
    fn build_api_routes_maps_rs_paths_to_patterns() {
        let root = mk_temp_root("build_api_routes");
        let src = root.join("src");

        write_file(&src.join("api/index.rs"), "pub fn router() {}");
        write_file(&src.join("api/todos.rs"), "pub fn router() {}");
        write_file(&src.join("api/users/[id].rs"), "pub fn router() {}");

        let routes = build_api_routes(&src).expect("routes should build");
        let patterns = routes.iter().map(|r| r.pattern.as_str()).collect::<Vec<_>>();

        assert!(patterns.contains(&"/api"), "index.rs -> /api");
        assert!(patterns.contains(&"/api/todos"));
        assert!(patterns.contains(&"/api/users/:id"));
        assert!(routes.iter().any(|r| r.symbol == "api_todos"));
        assert!(routes.iter().any(|r| r.symbol == "api_users_id"));
        assert!(routes.iter().any(|r| r.module_path == "api::todos"));

        cleanup(&root);
    }

    #[test]
    fn build_api_routes_returns_empty_when_api_missing() {
        let root = mk_temp_root("missing_api");
        let src = root.join("src");
        fs::create_dir_all(&src).expect("create src");

        let routes = build_api_routes(&src).expect("expected empty route list");
        assert!(routes.is_empty());

        cleanup(&root);
    }

    #[test]
    fn build_api_symbol_normalizes_correctly() {
        assert_eq!(build_api_symbol("todos"), "api_todos");
        assert_eq!(build_api_symbol("users/[id]"), "api_users_id");
        assert_eq!(build_api_symbol("index"), "api_index");
        assert_eq!(build_api_symbol(""), "api_index");
    }

    #[test]
    fn build_module_path_strips_brackets() {
        assert_eq!(build_module_path("todos"), "api::todos");
        assert_eq!(build_module_path("users/[id]"), "api::users::id");
        assert_eq!(build_module_path(""), "api");
        assert_eq!(build_module_path("[...slug]"), "api::slug");
    }
}
