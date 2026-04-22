use crate::workspace::{display_path, ensure_contained, resolve_project};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
pub struct ScaffoldFile {
    pub path: String,
    pub action: ScaffoldAction,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaffoldAction {
    Create,
    Update,
    SkipExists,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScaffoldResult {
    pub dry_run: bool,
    pub files: Vec<ScaffoldFile>,
}

#[derive(Debug, Clone, Copy)]
pub struct ScaffoldRequest<'a> {
    pub kind: &'a str,
    pub name: &'a str,
    pub route_path: Option<&'a str>,
    pub target_dir: Option<&'a str>,
    pub options: Option<&'a Value>,
    pub dry_run: bool,
    pub overwrite: bool,
}

pub fn orchestrate_feature(
    current_root: &Path,
    project_root: Option<&str>,
    manifest_path: Option<&str>,
    request: ScaffoldRequest<'_>,
) -> Result<ScaffoldResult> {
    let resolved = resolve_project(current_root, project_root, manifest_path)?;
    let mut files = match request.kind {
        "route" | "page" => scaffold_route(&resolved.app_root, request)?,
        "component" => scaffold_component(&resolved.app_root, request)?,
        "fragment" => scaffold_fragment(&resolved.app_root, request)?,
        "silcrow-form" | "silcrow" => scaffold_silcrow_form(&resolved.app_root, request)?,
        other => bail!("unsupported scaffold kind: {other}"),
    };

    for file in &mut files {
        let path = PathBuf::from(&file.path);
        ensure_contained(&resolved.app_root, &path)?;
        if path.exists() && !request.overwrite && !matches!(file.action, ScaffoldAction::Update) {
            file.action = ScaffoldAction::SkipExists;
            continue;
        }
        if !request.dry_run && !matches!(file.action, ScaffoldAction::SkipExists) {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, &file.content)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
    }

    Ok(ScaffoldResult {
        dry_run: request.dry_run,
        files,
    })
}

fn scaffold_route(app_root: &Path, request: ScaffoldRequest<'_>) -> Result<Vec<ScaffoldFile>> {
    let route_path = request.route_path.unwrap_or(request.name);
    let html_path = route_file(app_root, route_path, "html");
    let title = titleize(request.name);
    let with_code = option_bool(request.options, "code_behind").unwrap_or(true);
    let with_action = option_bool(request.options, "action").unwrap_or(false);
    let mut files = vec![ScaffoldFile {
        path: display_path(&html_path),
        action: ScaffoldAction::Create,
        content: format!(
            "<Fragment slot=\"title\"><title>{title}</title></Fragment>\n\n<h1>{title}</h1>\n"
        ),
    }];
    if with_code || with_action {
        let rs_path = route_file(app_root, route_path, "rs");
        let mut content = format!(
            "pub struct Props {{\n    pub title: &'static str,\n}}\n\npub async fn load(_req: Req) -> AppResult<Props> {{\n    Ok(Props {{ title: \"{title}\" }})\n}}\n"
        );
        if with_action {
            content.push_str(
                "\n// Named action invoked by POSTing to this page with `?/submit`.\npub async fn submit(req: Req) -> ActionResult {\n    req.res.no_cache();\n    redirect(&req.path)\n}\n",
            );
        }
        files.push(ScaffoldFile {
            path: display_path(&rs_path),
            action: ScaffoldAction::Create,
            content,
        });
    }
    Ok(files)
}

fn scaffold_component(app_root: &Path, request: ScaffoldRequest<'_>) -> Result<Vec<ScaffoldFile>> {
    let dir = request
        .target_dir
        .map(|dir| app_root.join(dir))
        .unwrap_or_else(|| app_root.join("src/ui"));
    let path = dir.join(format!("{}.html", pascal_name(request.name)));
    Ok(vec![ScaffoldFile {
        path: display_path(&path),
        action: ScaffoldAction::Create,
        content: format!(
            "<section class=\"{}\">\n    {{{{ slot }}}}\n</section>\n",
            kebab_name(request.name)
        ),
    }])
}

fn scaffold_fragment(app_root: &Path, request: ScaffoldRequest<'_>) -> Result<Vec<ScaffoldFile>> {
    let pilcrow_path = app_root.join("Pilcrow.toml");
    let source = fs::read_to_string(&pilcrow_path).unwrap_or_default();
    let has_fragment_config = source.contains("[[fragments]]");
    let fragment_dir = configured_fragment_dir(&source).unwrap_or_else(|| "widgets".to_string());
    let target_dir = request
        .target_dir
        .map(|dir| app_root.join(dir))
        .unwrap_or_else(|| app_root.join("src").join(&fragment_dir));
    let path = target_dir.join(format!("{}.html", kebab_name(request.name)));
    let mut files = vec![ScaffoldFile {
        path: display_path(&path),
        action: ScaffoldAction::Create,
        content: format!(
            "<article id=\"{}\">\n    {{{{ slot }}}}\n</article>\n",
            kebab_name(request.name)
        ),
    }];
    if !has_fragment_config {
        let updated = if source.trim().is_empty() {
            "[[fragments]]\ndir = \"widgets\"\n".to_string()
        } else {
            format!(
                "{}\n\n[[fragments]]\ndir = \"widgets\"\n",
                source.trim_end()
            )
        };
        files.push(ScaffoldFile {
            path: display_path(&pilcrow_path),
            action: ScaffoldAction::Update,
            content: updated,
        });
    }
    Ok(files)
}

fn scaffold_silcrow_form(
    app_root: &Path,
    request: ScaffoldRequest<'_>,
) -> Result<Vec<ScaffoldFile>> {
    let route_path = request.route_path.unwrap_or(request.name);
    let html_path = route_file(app_root, route_path, "html");
    let rs_path = route_file(app_root, route_path, "rs");
    let title = titleize(request.name);
    Ok(vec![
        ScaffoldFile {
            path: display_path(&html_path),
            action: ScaffoldAction::Create,
            content: format!(
                "<Fragment slot=\"title\"><title>{title}</title></Fragment>\n\n<h1>{title}</h1>\n<form method=\"post\" action=\"?/submit\" s-target=\"#form-result\">\n    <label>\n        Name\n        <input name=\"name\" value=\"{{{{ name }}}}\" />\n    </label>\n    <button type=\"submit\">Submit</button>\n</form>\n<div id=\"form-result\">{{{{ message }}}}</div>\n"
            ),
        },
        ScaffoldFile {
            path: display_path(&rs_path),
            action: ScaffoldAction::Create,
            content: "pub struct Props {\n    pub name: String,\n    pub message: String,\n}\n\npub async fn load(_req: Req) -> AppResult<Props> {\n    Ok(Props {\n        name: String::new(),\n        message: String::new(),\n    })\n}\n\npub async fn submit(req: Req) -> ActionResult {\n    let name = req.form.get(\"name\").unwrap_or(\"\").to_string();\n    req.res.no_cache().patch_target(\"#form-result\", &format!(\"Thanks, {}\", name));\n    redirect(&req.path)\n}\n".to_string(),
        },
    ])
}

fn route_file(app_root: &Path, route_path: &str, ext: &str) -> PathBuf {
    let cleaned = route_path
        .trim()
        .trim_start_matches('/')
        .trim_end_matches('/');
    if cleaned.is_empty() {
        app_root.join("src/pages").join(format!("index.{ext}"))
    } else {
        app_root
            .join("src/pages")
            .join(cleaned)
            .join(format!("index.{ext}"))
    }
}

fn option_bool(options: Option<&Value>, key: &str) -> Option<bool> {
    options
        .and_then(|options| options.get(key))
        .and_then(Value::as_bool)
}

fn configured_fragment_dir(source: &str) -> Option<String> {
    let value = toml::from_str::<toml::Value>(source).ok()?;
    let fragments = value.get("fragments")?.as_array()?;
    fragments
        .first()?
        .get("dir")?
        .as_str()
        .map(|dir| dir.trim_start_matches("src/").to_string())
}

fn titleize(input: &str) -> String {
    input
        .split(['-', '_', '/', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn pascal_name(input: &str) -> String {
    let title = titleize(input);
    title.replace(' ', "")
}

fn kebab_name(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_scaffold_defaults_to_dry_run_files() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("app")).unwrap();
        fs::write(
            temp.path().join("app/Cargo.toml"),
            "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
        )
        .unwrap();
        let result = orchestrate_feature(
            temp.path(),
            Some(temp.path().to_str().unwrap()),
            Some("app/Cargo.toml"),
            ScaffoldRequest {
                kind: "route",
                name: "reports",
                route_path: Some("/reports"),
                target_dir: None,
                options: None,
                dry_run: true,
                overwrite: false,
            },
        )
        .unwrap();
        assert_eq!(result.files.len(), 2);
        assert!(result.files[0]
            .path
            .ends_with("src/pages/reports/index.html"));
    }

    #[test]
    fn existing_file_is_skipped_without_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let app = temp.path().join("app");
        fs::create_dir_all(app.join("src/pages/reports")).unwrap();
        fs::write(
            app.join("Cargo.toml"),
            "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
        )
        .unwrap();
        fs::write(app.join("src/pages/reports/index.html"), "old").unwrap();
        let result = orchestrate_feature(
            temp.path(),
            Some(temp.path().to_str().unwrap()),
            Some("app/Cargo.toml"),
            ScaffoldRequest {
                kind: "route",
                name: "reports",
                route_path: Some("/reports"),
                target_dir: None,
                options: None,
                dry_run: false,
                overwrite: false,
            },
        )
        .unwrap();
        assert!(matches!(result.files[0].action, ScaffoldAction::SkipExists));
        assert_eq!(
            fs::read_to_string(app.join("src/pages/reports/index.html")).unwrap(),
            "old"
        );
    }
}
