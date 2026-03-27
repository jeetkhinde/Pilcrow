use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::codegen::{GeneratedPageRoute, write_generated_routes_module};
use crate::compiler::transpile_html_module;
use crate::discovery::discover_html_files;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlSourceKind {
    Page,
    Component,
    Layout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessedHtmlFile {
    pub kind: HtmlSourceKind,
    pub source_path: PathBuf,
    pub template_output_path: PathBuf,
    pub rust_frontmatter: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerOutput {
    pub preprocessed_files: Vec<PreprocessedHtmlFile>,
    pub generated_routes_file: PathBuf,
    pub generated_routes: Vec<GeneratedPageRoute>,
}

/// Full compile pipeline for Pilcrow `.html` sources.
///
/// Output layout in `out_dir`:
/// - `generated_routes.rs` (route manifest + registration helpers)
/// - `pilcrow_templates/{pages,components,layouts}/...` (transpiled Askama templates)
pub fn compile_to_out_dir(
    src_root: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
) -> io::Result<CompilerOutput> {
    let src_root = src_root.as_ref();
    let out_dir = out_dir.as_ref();

    let discovered = discover_html_files(src_root)?;
    let templates_root = out_dir.join("pilcrow_templates");

    let mut files = Vec::new();
    preprocess_group(
        HtmlSourceKind::Page,
        &discovered.pages,
        &src_root.join("pages"),
        &templates_root.join("pages"),
        &mut files,
    )?;
    preprocess_group(
        HtmlSourceKind::Component,
        &discovered.components,
        &src_root.join("components"),
        &templates_root.join("components"),
        &mut files,
    )?;
    preprocess_group(
        HtmlSourceKind::Layout,
        &discovered.layouts,
        &src_root.join("layouts"),
        &templates_root.join("layouts"),
        &mut files,
    )?;

    files.sort_by(|a, b| {
        a.template_output_path
            .cmp(&b.template_output_path)
            .then_with(|| a.source_path.cmp(&b.source_path))
    });

    let generated_routes_file = out_dir.join("generated_routes.rs");
    let generated_routes = write_generated_routes_module(src_root, &generated_routes_file)?;

    Ok(CompilerOutput {
        preprocessed_files: files,
        generated_routes_file,
        generated_routes,
    })
}

/// Canonical directories that should trigger rebuilds in Cargo build scripts.
pub fn watched_source_directories(src_root: impl AsRef<Path>) -> [PathBuf; 3] {
    let src_root = src_root.as_ref();
    [
        src_root.join("pages"),
        src_root.join("components"),
        src_root.join("layouts"),
    ]
}

fn preprocess_group(
    kind: HtmlSourceKind,
    source_files: &[PathBuf],
    source_root: &Path,
    out_root: &Path,
    out: &mut Vec<PreprocessedHtmlFile>,
) -> io::Result<()> {
    for source_path in source_files {
        let source = fs::read_to_string(source_path)?;
        let parts = transpile_html_module(&source).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse {}: {err}", source_path.display()),
            )
        })?;

        let relative = source_path.strip_prefix(source_root).unwrap_or(source_path);
        let template_output_path = out_root.join(relative);

        if let Some(parent) = template_output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&template_output_path, parts.template.as_bytes())?;

        out.push(PreprocessedHtmlFile {
            kind,
            source_path: source_path.clone(),
            template_output_path,
            rust_frontmatter: parts.rust,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn compile_pipeline_writes_transpiled_templates_and_routes() {
        let root = mk_temp_root("compile_pipeline_ok");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/index.html"),
            r#"---
pub struct Props {
    pub title: String,
}
---
<Layout title={title}>
    <Card title={title} />
</Layout>"#,
        );
        write_file(
            &src.join("components/Card.html"),
            r#"---
pub struct Props {
    pub title: String,
}
---
<article>{{ title }}</article>"#,
        );
        write_file(
            &src.join("layouts/Layout.html"),
            r#"---
pub struct Props {
    pub title: String,
}
---
<html><body>{{ title }}</body></html>"#,
        );

        let result = compile_to_out_dir(&src, &out).expect("pipeline should compile");

        assert_eq!(result.preprocessed_files.len(), 3);
        assert!(result.generated_routes_file.exists());
        assert!(result.generated_routes.iter().any(|r| r.pattern == "/"));

        let page_template = out.join("pilcrow_templates/pages/index.html");
        let page_rendered = fs::read_to_string(page_template).expect("read transpiled page");
        assert!(page_rendered.contains("<Layout title={title}>"));
        assert!(page_rendered.contains("{{ Card { title: title }|safe }}"));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_fails_on_invalid_html_module() {
        let root = mk_temp_root("compile_pipeline_bad");
        let src = root.join("src");
        let out = root.join("out");

        write_file(&src.join("pages/index.html"), "<h1>Missing fences</h1>");

        let err = compile_to_out_dir(&src, &out).expect_err("pipeline should fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);

        cleanup(&root);
    }

    #[test]
    fn watched_dirs_are_pages_components_layouts() {
        let src = PathBuf::from("/tmp/project/src");
        let dirs = watched_source_directories(&src);
        assert_eq!(dirs[0], PathBuf::from("/tmp/project/src/pages"));
        assert_eq!(dirs[1], PathBuf::from("/tmp/project/src/components"));
        assert_eq!(dirs[2], PathBuf::from("/tmp/project/src/layouts"));
    }

    fn mk_temp_root(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pilcrow_routekit_pipeline_{}_{}_{}",
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
