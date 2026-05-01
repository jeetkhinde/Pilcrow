use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::routing::discovery::IgnoreFilter;
use crate::templating::build_config::{ReactBuildConfig, RoutingConfig};

pub const REACT_ENTRY_PLACEHOLDER_PREFIX: &str = "__PILCROW_REACT_ENTRY_";
pub const REACT_CSS_PLACEHOLDER_PREFIX: &str = "__PILCROW_REACT_CSS_";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactIslandRef {
    pub id: String,
    pub source_path: PathBuf,
}

#[derive(Debug, Clone)]
struct ReactSource {
    id: String,
    source_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ViteManifestEntry {
    file: String,
    #[serde(default)]
    css: Vec<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    src: Option<String>,
    #[serde(default, rename = "isEntry")]
    is_entry: bool,
}

pub fn transpile_react_tags(
    template: &str,
    template_path: &Path,
    src_root: &Path,
    react_config: &ReactBuildConfig,
    routing: &RoutingConfig,
) -> io::Result<(String, Vec<ReactIslandRef>)> {
    let mut output = String::with_capacity(template.len());
    let mut refs = Vec::new();
    let mut i = 0usize;
    let mut counter = 0usize;

    while i < template.len() {
        if template[i..].starts_with("<react") {
            let rest = &template[i + 6..];
            let next = rest.chars().next();
            if matches!(next, Some(c) if c.is_whitespace() || c == '/' || c == '>') {
                let (html, consumed, island) = parse_react_tag(
                    &template[i..],
                    template_path,
                    src_root,
                    react_config,
                    routing,
                    counter,
                )?;
                output.push_str(&html);
                refs.push(island);
                i += consumed;
                counter += 1;
                continue;
            }
        }
        let c = template[i..].chars().next().unwrap();
        output.push(c);
        i += c.len_utf8();
    }

    Ok((output, refs))
}

fn parse_react_tag(
    input: &str,
    template_path: &Path,
    src_root: &Path,
    react_config: &ReactBuildConfig,
    routing: &RoutingConfig,
    tag_index: usize,
) -> io::Result<(String, usize, ReactIslandRef)> {
    debug_assert!(input.starts_with("<react"));

    let (raw_attrs, consumed) = consume_tag_attrs(input, "react")?;
    let attrs = parse_attrs(&raw_attrs);
    let src = required_attr(&attrs, "src", template_path)?;
    let strategy = required_attr(&attrs, "strategy", template_path)?;
    if !matches!(strategy.as_str(), "load" | "visible" | "idle") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "`strategy` on <react> in {} must be one of: load, visible, idle",
                template_path.display()
            ),
        ));
    }

    let source_path = resolve_react_source(&src, template_path, src_root)?;
    if !source_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "<react src=\"{}\"> in {} resolved to {}, but that file does not exist",
                src,
                template_path.display(),
                source_path.display()
            ),
        ));
    }

    validate_react_source_directory(&source_path, src_root, react_config, routing)?;

    let id = react_id(template_path, tag_index);
    let entry_placeholder = format!("{REACT_ENTRY_PLACEHOLDER_PREFIX}{id}__");
    let css_placeholder = format!("{REACT_CSS_PLACEHOLDER_PREFIX}{id}__");
    let mut html = format!(
        "<div data-pilcrow-react data-id=\"{}\" data-src=\"{}\" data-strategy=\"{}\" data-css=\"{}\"",
        html_escape_attr(&id),
        html_escape_attr(&entry_placeholder),
        html_escape_attr(&strategy),
        html_escape_attr(&css_placeholder),
    );

    for (name, value) in attrs {
        if name == "src" || name == "strategy" {
            continue;
        }
        if value.is_empty() {
            html.push_str(&format!(" data-prop-{}=\"true\"", html_escape_attr(&name)));
        } else {
            html.push_str(&format!(
                " data-prop-{}=\"{}\"",
                html_escape_attr(&name),
                html_escape_attr(&value)
            ));
        }
    }
    html.push_str("></div><script type=\"module\" src=\"{{ pilcrow_web::assets::assets::react_islands_js_path() }}\"></script>");

    Ok((html, consumed, ReactIslandRef { id, source_path }))
}

pub fn build_react_assets(
    manifest_dir: &Path,
    out_dir: &Path,
    islands: &[ReactIslandRef],
    react_config: &ReactBuildConfig,
) -> io::Result<HashMap<String, (String, Vec<String>)>> {
    write_empty_react_assets(out_dir)?;
    if islands.is_empty() {
        return Ok(HashMap::new());
    }
    if !react_config.enabled {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "<react> tags were found, but [client.react].enabled is not true in Pilcrow.toml",
        ));
    }

    let mut sources_by_id = BTreeMap::new();
    for island in islands {
        sources_by_id
            .entry(island.id.clone())
            .or_insert_with(|| ReactSource {
                id: island.id.clone(),
                source_path: island.source_path.clone(),
            });
    }

    let react_root = out_dir.join("pilcrow_react");
    let entries_dir = react_root.join("entries");
    let dist_dir = react_root.join("dist");
    let hooks_path = react_root.join("pilcrow_react_hooks.jsx");
    fs::create_dir_all(&entries_dir)?;
    fs::create_dir_all(&dist_dir)?;
    fs::write(&hooks_path, render_hooks_module())?;

    let mut inputs = BTreeMap::new();
    for source in sources_by_id.values() {
        let entry_path = entries_dir.join(format!("{}.tsx", source.id));
        fs::write(
            &entry_path,
            render_entry_wrapper(&source.id, &source.source_path),
        )?;
        inputs.insert(source.id.clone(), entry_path);
    }

    let config_path = react_root.join("vite.config.mjs");
    fs::write(&config_path, render_vite_config(&inputs, &dist_dir, &hooks_path))?;
    run_vite(manifest_dir, &config_path)?;

    let manifest_path = dist_dir.join(".vite/manifest.json");
    let manifest_raw = fs::read_to_string(&manifest_path).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "React island build completed but {} could not be read: {err}",
                manifest_path.display()
            ),
        )
    })?;
    let manifest: HashMap<String, ViteManifestEntry> = serde_json::from_str(&manifest_raw)
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "failed to parse Vite manifest {}: {err}",
                    manifest_path.display()
                ),
            )
        })?;

    let mut id_to_urls = HashMap::new();
    for source in sources_by_id.values() {
        let manifest_key = format!("{}.tsx", source.id);
        let entry = manifest
            .get(&manifest_key)
            .or_else(|| {
                manifest.iter().find_map(|(key, entry)| {
                    let key_matches = key.ends_with(&format!("/{}", manifest_key));
                    let src_matches = entry
                        .src
                        .as_deref()
                        .is_some_and(|src| src.ends_with(&format!("/{}", manifest_key)));
                    let name_matches = entry.name.as_deref() == Some(source.id.as_str());
                    if entry.is_entry && (key_matches || src_matches || name_matches) {
                        Some(entry)
                    } else {
                        None
                    }
                })
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Vite manifest did not contain React island entry `{manifest_key}`"),
                )
            })?;
        id_to_urls.insert(
            source.id.clone(),
            (
                format!("/_pilcrow/client/{}", entry.file),
                entry
                    .css
                    .iter()
                    .map(|css| format!("/_pilcrow/client/{css}"))
                    .collect::<Vec<_>>(),
            ),
        );
    }

    check_entry_budgets(&dist_dir, &id_to_urls, react_config)?;
    write_react_assets_module(out_dir, &dist_dir)?;
    check_budgets(&dist_dir, react_config)?;
    Ok(id_to_urls)
}

pub fn replace_react_placeholders(
    html: &str,
    id_to_urls: &HashMap<String, (String, Vec<String>)>,
) -> String {
    let mut out = html.to_string();
    for (id, (entry, css)) in id_to_urls {
        out = out.replace(
            &format!("{REACT_ENTRY_PLACEHOLDER_PREFIX}{id}__"),
            entry.as_str(),
        );
        out = out.replace(
            &format!("{REACT_CSS_PLACEHOLDER_PREFIX}{id}__"),
            &css.join(","),
        );
    }
    out
}

fn run_vite(manifest_dir: &Path, config_path: &Path) -> io::Result<()> {
    let vite_bin = if cfg!(windows) {
        manifest_dir.join("node_modules/.bin/vite.cmd")
    } else {
        manifest_dir.join("node_modules/.bin/vite")
    };
    if !vite_bin.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "React islands require local Vite dependencies. Add package.json dependencies for vite, react, react-dom, and run npm install in {}",
                manifest_dir.display()
            ),
        ));
    }

    let status = Command::new(vite_bin)
        .arg("build")
        .arg("--config")
        .arg(config_path)
        .current_dir(manifest_dir)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "React island Vite build failed",
        ))
    }
}

fn render_entry_wrapper(id: &str, source_path: &Path) -> String {
    let src = source_path.to_string_lossy().replace('\\', "/");
    let id_json = serde_json::to_string(id).unwrap();
    format!(
        r#"import React from "react";
import {{ createRoot }} from "react-dom/client";
import Component from "{src}";

export function mount(el, props) {{
  if (el.__pilcrowReactRoot) {{
    el.__pilcrowReactRoot.render(React.createElement(Component, props));
    return;
  }}
  const root = createRoot(el);
  el.__pilcrowReactRoot = root;
  root.render(React.createElement(Component, props));
}}

window.__pilcrowReactMounts = window.__pilcrowReactMounts || {{}};
window.__pilcrowReactMounts[{id_json}] = mount;
"#
    )
}

fn render_hooks_module() -> &'static str {
    r#"import { use, useActionState, useMemo, useSyncExternalStore } from "react";

function silcrow() {
  if (!window.Silcrow) {
    throw new Error("Silcrow is not loaded. Include pilcrow_web::assets::assets::script_tag() in the page layout.");
  }
  return window.Silcrow;
}

export function silcrowRouteScope(path) {
  try {
    return `route:${new URL(path, window.location.origin).pathname}`;
  } catch (_err) {
    return `route:${path}`;
  }
}

export function useSilcrowAtom(scope, initialValue) {
  return useSyncExternalStore(
    (notify) => window.Silcrow?.subscribe(scope, notify) ?? (() => {}),
    () => window.Silcrow?.snapshot(scope) ?? initialValue,
    () => window.Silcrow?.snapshot(scope) ?? initialValue,
  );
}

export function publishSilcrowAtom(scope, data) {
  window.Silcrow?.publish(scope, data);
}

export function useSilcrowPrefetch(path) {
  return useMemo(() => silcrow().prefetch(path), [path]);
}

export function useSilcrowRoute(path, initialValue) {
  const promise = useSilcrowPrefetch(path);
  const initial = use(promise);
  return useSilcrowAtom(silcrowRouteScope(path), initial ?? initialValue);
}

export function useSilcrowAction(url, reducer, initialState, options = {}) {
  return useActionState(async (prev, body) => {
    const result = await silcrow().submit(url, body, {
      method: options.method ?? "POST",
      scope: options.scope,
    });
    return reducer ? reducer(result, prev) : (result.data ?? prev);
  }, initialState);
}
"#
}

fn render_vite_config(
    inputs: &BTreeMap<String, PathBuf>,
    dist_dir: &Path,
    hooks_path: &Path,
) -> String {
    let out = dist_dir.to_string_lossy().replace('\\', "/");
    let hooks = hooks_path.to_string_lossy().replace('\\', "/");
    let input = inputs
        .iter()
        .map(|(id, path)| {
            format!(
                "      {}: {},",
                serde_json::to_string(id).unwrap(),
                serde_json::to_string(&path.to_string_lossy().replace('\\', "/")).unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"import {{ createRequire }} from "node:module";

const require = createRequire(process.cwd() + "/package.json");

export default {{
  root: process.cwd(),
  resolve: {{
    alias: [
      {{ find: /^react$/, replacement: require.resolve("react") }},
      {{ find: /^react-dom\/client$/, replacement: require.resolve("react-dom/client") }},
      {{ find: /^react\/jsx-runtime$/, replacement: require.resolve("react/jsx-runtime") }},
      {{ find: /^pilcrow\/react$/, replacement: {} }}
    ]
  }},
  esbuild: {{ jsx: "automatic" }},
  build: {{
    outDir: {},
    emptyOutDir: true,
    manifest: true,
    rollupOptions: {{
      input: {{
{}
      }},
      output: {{
        entryFileNames: "[name]-[hash].js",
        chunkFileNames: "chunks/[name]-[hash].js",
        assetFileNames: "assets/[name]-[hash][extname]",
        manualChunks(id) {{
          if (id.includes("node_modules/react")) return "react";
        }}
      }}
    }}
  }}
}};
"#,
        serde_json::to_string(&hooks).unwrap(),
        serde_json::to_string(&out).unwrap(),
        input
    )
}

fn write_empty_react_assets(out_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(out_dir)?;
    fs::write(
        out_dir.join("generated_react_assets.rs"),
        r#"pub fn asset(_path: &str) -> Option<(&'static str, &'static [u8])> {
    None
}
"#,
    )
}

fn write_react_assets_module(out_dir: &Path, dist_dir: &Path) -> io::Result<()> {
    let mut files = Vec::new();
    collect_dist_files(dist_dir, dist_dir, &mut files)?;
    files.sort();

    let mut src = String::new();
    src.push_str("pub fn asset(path: &str) -> Option<(&'static str, &'static [u8])> {\n");
    src.push_str("    match path {\n");
    for rel in files {
        let rel_text = rel.to_string_lossy().replace('\\', "/");
        if rel_text == ".vite/manifest.json" {
            continue;
        }
        let abs = dist_dir.join(&rel);
        let abs_text = abs.to_string_lossy().replace('\\', "/");
        let content_type = content_type_for(&rel_text);
        src.push_str(&format!(
            "        {:?} => Some(({:?}, include_bytes!({:?}) as &'static [u8])),\n",
            rel_text, content_type, abs_text
        ));
    }
    src.push_str("        _ => None,\n");
    src.push_str("    }\n");
    src.push_str("}\n");
    fs::write(out_dir.join("generated_react_assets.rs"), src)
}

fn collect_dist_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_dist_files(root, &path, out)?;
        } else if file_type.is_file() {
            out.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
        }
    }
    Ok(())
}

fn check_budgets(dist_dir: &Path, react_config: &ReactBuildConfig) -> io::Result<()> {
    if !react_config.warn && !react_config.fail_on_budget {
        return Ok(());
    }
    let mut files = Vec::new();
    collect_dist_files(dist_dir, dist_dir, &mut files)?;
    let total_js = files
        .iter()
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("js"))
        .filter_map(|rel| fs::metadata(dist_dir.join(rel)).ok())
        .map(|meta| meta.len())
        .sum::<u64>();
    if let Some(limit_kb) = react_config.max_page_react_kb {
        let limit = limit_kb * 1024;
        if total_js > limit {
            let msg = format!(
                "React island JS is {} KB, above max_page_react_kb = {}",
                total_js / 1024,
                limit_kb
            );
            if react_config.fail_on_budget {
                return Err(io::Error::new(io::ErrorKind::Other, msg));
            }
            println!("cargo:warning={msg}");
        }
    }
    Ok(())
}

fn check_entry_budgets(
    dist_dir: &Path,
    id_to_urls: &HashMap<String, (String, Vec<String>)>,
    react_config: &ReactBuildConfig,
) -> io::Result<()> {
    if !react_config.warn && !react_config.fail_on_budget {
        return Ok(());
    }
    let Some(limit_kb) = react_config.max_island_kb else {
        return Ok(());
    };
    let limit = limit_kb * 1024;
    for (id, (entry_url, _)) in id_to_urls {
        let rel = entry_url.trim_start_matches("/_pilcrow/client/");
        let size = fs::metadata(dist_dir.join(rel))
            .map(|meta| meta.len())
            .unwrap_or(0);
        if size > limit {
            let msg = format!(
                "React island `{id}` entry is {} KB, above max_island_kb = {}",
                size / 1024,
                limit_kb
            );
            if react_config.fail_on_budget {
                return Err(io::Error::new(io::ErrorKind::Other, msg));
            }
            println!("cargo:warning={msg}");
        }
    }
    Ok(())
}

fn content_type_for(path: &str) -> &'static str {
    if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    }
}

fn resolve_react_source(src: &str, template_path: &Path, src_root: &Path) -> io::Result<PathBuf> {
    let path = if src.starts_with('/') {
        src_root.join(src.trim_start_matches('/'))
    } else {
        template_path.parent().unwrap_or(src_root).join(src)
    };
    path.canonicalize().or_else(|_| Ok(normalize_path(path)))
}

fn validate_react_source_directory(
    source_path: &Path,
    src_root: &Path,
    react_config: &ReactBuildConfig,
    routing: &RoutingConfig,
) -> io::Result<()> {
    let canonical_src_root = src_root
        .canonicalize()
        .unwrap_or_else(|_| normalize_path(src_root.to_path_buf()));
    let canonical_source = source_path
        .canonicalize()
        .unwrap_or_else(|_| normalize_path(source_path.to_path_buf()));
    let source_rel = canonical_source
        .strip_prefix(&canonical_src_root)
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "React source {} must live under {}",
                    canonical_source.display(),
                    canonical_src_root.display()
                ),
            )
        })?;
    let filter = IgnoreFilter::new(&routing.ignore_directories);
    let dirs = if react_config.dirs.is_empty() {
        vec!["react".to_string()]
    } else {
        react_config.dirs.clone()
    };
    let mut current = canonical_source.parent();
    while let Some(dir) = current {
        if dir == canonical_src_root {
            break;
        }
        let Some(name) = dir.file_name().and_then(|name| name.to_str()) else {
            current = dir.parent();
            continue;
        };
        if dirs.iter().any(|allowed| allowed == name) {
            let rel = dir.strip_prefix(&canonical_src_root).unwrap_or(dir);
            if filter.is_ignored(rel) {
                return Ok(());
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "React source {} is in `{name}`, but that directory is not ignored by [routing].ignore_directories. Add `ignore_directories = [\"{name}\"]`.",
                    source_rel.display()
                ),
            ));
        }
        current = dir.parent();
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "React source {} must live inside one of [client.react].dirs: {:?}",
            source_rel.display(),
            dirs
        ),
    ))
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn react_id(template_path: &Path, tag_index: usize) -> String {
    let raw = format!("{}_{}", template_path.to_string_lossy(), tag_index);
    let mut out = String::from("react");
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push('_');
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_end_matches('_').to_string()
}

fn required_attr(
    attrs: &HashMap<String, String>,
    name: &str,
    template_path: &Path,
) -> io::Result<String> {
    attrs
        .get(name)
        .filter(|v| !v.is_empty())
        .cloned()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("<react> in {} requires `{name}`", template_path.display()),
            )
        })
}

fn consume_tag_attrs(input: &str, tag: &str) -> io::Result<(String, usize)> {
    let mut idx = tag.len() + 1;
    let mut raw_attrs = String::new();
    let mut quote: Option<char> = None;
    let mut brace_depth = 0usize;
    while idx < input.len() {
        let c = input[idx..].chars().next().unwrap();
        let c_len = c.len_utf8();
        if let Some(q) = quote {
            raw_attrs.push(c);
            if c == q {
                quote = None;
            }
            idx += c_len;
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                raw_attrs.push(c);
                idx += c_len;
            }
            '{' => {
                brace_depth += 1;
                raw_attrs.push(c);
                idx += c_len;
            }
            '}' => {
                brace_depth = brace_depth.saturating_sub(1);
                raw_attrs.push(c);
                idx += c_len;
            }
            '/' if brace_depth == 0 => {
                idx += c_len;
                if input[idx..].starts_with('>') {
                    return Ok((raw_attrs, idx + 1));
                }
                raw_attrs.push('/');
            }
            '>' if brace_depth == 0 => return Ok((raw_attrs, idx + c_len)),
            _ => {
                raw_attrs.push(c);
                idx += c_len;
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "unterminated <react> tag",
    ))
}

fn parse_attrs(raw: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    let mut i = 0usize;
    let bytes = raw.as_bytes();
    while i < raw.len() {
        while i < raw.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= raw.len() {
            break;
        }
        let name_start = i;
        while i < raw.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'/'
        {
            i += 1;
        }
        let name = raw[name_start..i].trim().to_string();
        while i < raw.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= raw.len() || bytes[i] != b'=' {
            attrs.insert(name, String::new());
            continue;
        }
        i += 1;
        while i < raw.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = if i < raw.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
            let quote = bytes[i];
            i += 1;
            let value_start = i;
            while i < raw.len() && bytes[i] != quote {
                i += 1;
            }
            let value = raw[value_start..i].to_string();
            if i < raw.len() {
                i += 1;
            }
            value
        } else {
            let value_start = i;
            while i < raw.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            raw[value_start..i].to_string()
        };
        attrs.insert(name, value);
    }
    attrs
}

fn html_escape_attr(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templating::build_config::{ReactBuildConfig, RoutingConfig};

    #[test]
    fn react_tag_transpiles_to_inert_mount() {
        let root = std::env::temp_dir().join("pilcrow_react_tag_transpile");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src/pages/dashboard/react")).unwrap();
        let page = root.join("src/pages/dashboard/index.html");
        fs::write(
            root.join("src/pages/dashboard/react/Counter.tsx"),
            "export default function Counter() { return null }",
        )
        .unwrap();

        let (out, refs) = transpile_react_tags(
            r#"<react src="./react/Counter.tsx" strategy="visible" count="{{ props.count }}" />"#,
            &page,
            &root.join("src"),
            &ReactBuildConfig {
                enabled: true,
                dirs: vec!["react".into()],
                ..Default::default()
            },
            &RoutingConfig {
                ignore_directories: vec!["react".into()],
            },
        )
        .unwrap();

        assert!(out.contains("data-pilcrow-react"));
        assert!(out.contains("data-strategy=\"visible\""));
        assert!(out.contains("data-prop-count=\"{{ props.count }}\""));
        assert_eq!(refs.len(), 1);
    }

    #[test]
    fn react_tag_requires_ignored_directory() {
        let root = std::env::temp_dir().join("pilcrow_react_tag_ignore");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src/pages/dashboard/react")).unwrap();
        let page = root.join("src/pages/dashboard/index.html");
        fs::write(
            root.join("src/pages/dashboard/react/Counter.tsx"),
            "export default function Counter() { return null }",
        )
        .unwrap();

        let err = transpile_react_tags(
            r#"<react src="./react/Counter.tsx" strategy="visible" />"#,
            &page,
            &root.join("src"),
            &ReactBuildConfig {
                enabled: true,
                dirs: vec!["react".into()],
                ..Default::default()
            },
            &RoutingConfig::default(),
        )
        .unwrap_err();

        assert!(err.to_string().contains("ignore_directories"));
    }

    #[test]
    fn react_tag_accepts_jsx_source() {
        let root = std::env::temp_dir().join("pilcrow_react_tag_jsx");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src/pages/dashboard/react")).unwrap();
        let page = root.join("src/pages/dashboard/index.html");
        fs::write(
            root.join("src/pages/dashboard/react/Counter.jsx"),
            "export default function Counter() { return <button>Count</button> }",
        )
        .unwrap();

        let (out, refs) = transpile_react_tags(
            r#"<react src="./react/Counter.jsx" strategy="visible" />"#,
            &page,
            &root.join("src"),
            &ReactBuildConfig {
                enabled: true,
                dirs: vec!["react".into()],
                ..Default::default()
            },
            &RoutingConfig {
                ignore_directories: vec!["react".into()],
            },
        )
        .unwrap();

        assert!(out.contains("data-pilcrow-react"));
        assert_eq!(refs.len(), 1);
        assert!(refs[0].source_path.ends_with("Counter.jsx"));
    }

    #[test]
    fn vite_config_aliases_pilcrow_react_hooks() {
        let mut inputs = BTreeMap::new();
        inputs.insert("counter".to_string(), PathBuf::from("/tmp/counter.tsx"));

        let config = render_vite_config(
            &inputs,
            Path::new("/tmp/dist"),
            Path::new("/tmp/pilcrow_react_hooks.jsx"),
        );

        assert!(config.contains("find: /^pilcrow\\/react$/"));
        assert!(config.contains(r#"replacement: "/tmp/pilcrow_react_hooks.jsx""#));
        assert!(config.contains(r#"outDir: "/tmp/dist""#));
    }

    #[test]
    fn react_hooks_module_exposes_hook_api() {
        let hooks = render_hooks_module();

        assert!(hooks.contains("export function useSilcrowAtom"));
        assert!(hooks.contains("export function useSilcrowPrefetch"));
        assert!(hooks.contains("export function useSilcrowRoute"));
        assert!(hooks.contains("export function useSilcrowAction"));
        assert!(hooks.contains("export function publishSilcrowAtom"));
    }
}
