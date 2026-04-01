use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::Route;

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
}
