use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::codegen::{
    GeneratedPageRoute, GeneratedTemplateEntry, TemplateCodegenInput,
    write_generated_routes_module, write_generated_templates_module,
};
use crate::compiler::{split_html_module, transpile_component_tags};
use crate::discovery::{DiscoveredHtmlFiles, discover_html_files};

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
    pub transpiled_template: String,
    pub module_name: String,
    pub render_symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerOutput {
    pub preprocessed_files: Vec<PreprocessedHtmlFile>,
    pub generated_routes_file: PathBuf,
    pub generated_routes: Vec<GeneratedPageRoute>,
    pub generated_templates_file: PathBuf,
    pub generated_templates: Vec<GeneratedTemplateEntry>,
}

/// Full compile pipeline for Pilcrow `.html` sources.
///
/// Output layout in `out_dir`:
/// - `generated_routes.rs` (route manifest + registration helpers)
/// - `generated_templates.rs` (compile-time Askama render functions)
/// - `pilcrow_templates/{pages,components,layouts}/...` (transpiled Askama templates)
pub fn compile_to_out_dir(
    src_root: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
) -> io::Result<CompilerOutput> {
    let src_root = src_root.as_ref();
    let out_dir = out_dir.as_ref();

    let discovered = discover_html_files(src_root)?;
    let component_registry = build_component_registry(&discovered)?;
    let templates_root = out_dir.join("pilcrow_templates");

    let mut files = Vec::new();
    preprocess_group(
        HtmlSourceKind::Page,
        &discovered.pages,
        &src_root.join("pages"),
        &templates_root.join("pages"),
        &component_registry,
        &mut files,
    )?;
    preprocess_group(
        HtmlSourceKind::Component,
        &discovered.components,
        &src_root.join("components"),
        &templates_root.join("components"),
        &component_registry,
        &mut files,
    )?;
    preprocess_group(
        HtmlSourceKind::Layout,
        &discovered.layouts,
        &src_root.join("layouts"),
        &templates_root.join("layouts"),
        &component_registry,
        &mut files,
    )?;

    files.sort_by(|a, b| {
        a.template_output_path
            .cmp(&b.template_output_path)
            .then_with(|| a.source_path.cmp(&b.source_path))
    });

    let generated_routes_file = out_dir.join("generated_routes.rs");
    let generated_routes = write_generated_routes_module(src_root, &generated_routes_file)?;
    let generated_templates_file = out_dir.join("generated_templates.rs");

    let template_codegen_inputs = files
        .iter()
        .map(|file| TemplateCodegenInput {
            module_name: file.module_name.clone(),
            render_symbol: file.render_symbol.clone(),
            source_path: normalize_path_text(&file.source_path),
            rust_frontmatter: file.rust_frontmatter.clone(),
            template_source: file.transpiled_template.clone(),
        })
        .collect::<Vec<_>>();
    let generated_templates =
        write_generated_templates_module(&template_codegen_inputs, &generated_templates_file)?;

    Ok(CompilerOutput {
        preprocessed_files: files,
        generated_routes_file,
        generated_routes,
        generated_templates_file,
        generated_templates,
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
    component_registry: &HashMap<String, String>,
    out: &mut Vec<PreprocessedHtmlFile>,
) -> io::Result<()> {
    for source_path in source_files {
        let source = fs::read_to_string(source_path)?;
        let parts = split_html_module(&source).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse {}: {err}", source_path.display()),
            )
        })?;

        let mut stack = Vec::new();
        let expanded = expand_known_components(&parts.template, component_registry, &mut stack, 0);
        let final_template = transpile_component_tags(&expanded);

        let relative = source_path.strip_prefix(source_root).unwrap_or(source_path);
        let template_output_path = out_root.join(relative);
        let module_name = build_module_name(kind, relative);
        let render_symbol = format!("render_{module_name}");

        if let Some(parent) = template_output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&template_output_path, final_template.as_bytes())?;

        out.push(PreprocessedHtmlFile {
            kind,
            source_path: source_path.clone(),
            template_output_path,
            rust_frontmatter: parts.rust,
            transpiled_template: final_template,
            module_name,
            render_symbol,
        });
    }

    Ok(())
}

fn build_module_name(kind: HtmlSourceKind, relative: &Path) -> String {
    let prefix = match kind {
        HtmlSourceKind::Page => "page",
        HtmlSourceKind::Component => "component",
        HtmlSourceKind::Layout => "layout",
    };

    let relative = relative.to_string_lossy().replace('\\', "/");
    let without_ext = relative.strip_suffix(".html").unwrap_or(&relative);
    let mut symbol = String::new();
    let mut prev_underscore = false;
    for ch in without_ext.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };

        if mapped == '_' {
            if !prev_underscore {
                symbol.push('_');
            }
            prev_underscore = true;
        } else {
            symbol.push(mapped);
            prev_underscore = false;
        }
    }

    let symbol = symbol.trim_matches('_');
    if symbol.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}_{symbol}")
    }
}

fn normalize_path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn build_component_registry(
    discovered: &DiscoveredHtmlFiles,
) -> io::Result<HashMap<String, String>> {
    let mut registry = HashMap::new();

    let mut files = discovered
        .layouts
        .iter()
        .chain(discovered.components.iter())
        .cloned()
        .collect::<Vec<_>>();
    files.sort();

    for source_path in files {
        let source = fs::read_to_string(&source_path)?;
        let parts = split_html_module(&source).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse {}: {err}", source_path.display()),
            )
        })?;

        let Some(stem) = source_path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !is_pascal_case_name(stem) {
            continue;
        }

        registry.insert(stem.to_string(), parts.template);
    }

    Ok(registry)
}

fn expand_known_components(
    template: &str,
    registry: &HashMap<String, String>,
    stack: &mut Vec<String>,
    depth: usize,
) -> String {
    const MAX_DEPTH: usize = 64;
    if depth >= MAX_DEPTH {
        return template.to_string();
    }

    let mut out = String::with_capacity(template.len());
    let mut i = 0usize;

    while i < template.len() {
        let Some(ch) = template[i..].chars().next() else {
            break;
        };

        if ch == '<'
            && let Some(invocation) = parse_component_invocation(&template[i..])
            && let Some(component_template) = registry.get(&invocation.name)
        {
            if stack.iter().any(|name| name == &invocation.name) {
                out.push_str(&template[i..i + invocation.consumed]);
                i += invocation.consumed;
                continue;
            }

            let inner_expanded = invocation
                .inner
                .as_deref()
                .map(|inner| expand_known_components(inner, registry, stack, depth + 1))
                .unwrap_or_default();

            let slot_assignments = collect_slot_assignments(&inner_expanded);
            let component_with_slot = apply_slots(component_template, &slot_assignments);

            stack.push(invocation.name.clone());
            let component_body =
                expand_known_components(&component_with_slot, registry, stack, depth + 1);
            stack.pop();

            out.push_str(&render_askama_let_bindings(&invocation.attrs));
            out.push_str(&component_body);
            i += invocation.consumed;
            continue;
        }

        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

fn render_askama_let_bindings(attrs: &[(String, String)]) -> String {
    let mut out = String::new();
    for (name, expr) in attrs {
        let expr = expr.trim();
        if expr == name {
            continue;
        }
        out.push_str("{% let ");
        out.push_str(name);
        out.push_str(" = ");
        out.push('(');
        out.push_str(expr);
        out.push_str(").clone()");
        out.push_str(" %}");
    }
    out
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SlotAssignments {
    default: Vec<SlotFragment>,
    named: HashMap<String, Vec<SlotFragment>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SlotFragment {
    content: String,
    let_bindings: Vec<String>,
}

fn collect_slot_assignments(inner: &str) -> SlotAssignments {
    let mut assignments = SlotAssignments::default();
    let mut idx = 0usize;

    while idx < inner.len() {
        if let Some(node) = parse_html_node_at(inner, idx) {
            if let Some(slot_name) = extract_slot_name(&node.attrs) {
                let let_bindings = extract_slot_let_bindings(&node.attrs);
                let content = render_slot_fragment_node(inner, &node);
                let fragment = SlotFragment {
                    content,
                    let_bindings,
                };

                if slot_name == "default" {
                    assignments.default.push(fragment);
                } else {
                    assignments
                        .named
                        .entry(slot_name)
                        .or_default()
                        .push(fragment);
                }
            } else {
                assignments.default.push(SlotFragment {
                    content: inner[idx..idx + node.consumed].to_string(),
                    let_bindings: Vec::new(),
                });
            }
            idx += node.consumed;
            continue;
        }

        // Text chunk until next potential tag
        let next_tag = inner[idx..]
            .find('<')
            .map(|off| idx + off)
            .unwrap_or(inner.len());
        if next_tag > idx {
            assignments.default.push(SlotFragment {
                content: inner[idx..next_tag].to_string(),
                let_bindings: Vec::new(),
            });
        }
        idx = next_tag.max(idx + 1);
    }

    assignments
}

fn render_slot_fragment_node(source: &str, node: &HtmlNode) -> String {
    if node.name.eq_ignore_ascii_case("Fragment") {
        return node
            .inner
            .map(|(start, end)| source[start..end].to_string())
            .unwrap_or_default();
    }

    let attrs = node
        .attrs
        .iter()
        .filter(|a| a.name != "slot" && !a.name.starts_with("let:"))
        .map(render_html_attr)
        .collect::<String>();

    if node.self_closing {
        return format!("<{}{} />", node.name, attrs);
    }

    let inner = node
        .inner
        .map(|(start, end)| &source[start..end])
        .unwrap_or_default();
    format!("<{}{}>{}</{}>", node.name, attrs, inner, node.name)
}

fn apply_slots(component_template: &str, assignments: &SlotAssignments) -> String {
    let mut out = String::new();
    let mut idx = 0usize;

    while idx < component_template.len() {
        if let Some(slot_tag) = parse_slot_tag_at(component_template, idx) {
            let replacement = render_slot_replacement(assignments, &slot_tag);
            out.push_str(&replacement);
            idx += slot_tag.consumed;
            continue;
        }

        let Some(ch) = component_template[idx..].chars().next() else {
            break;
        };
        out.push(ch);
        idx += ch.len_utf8();
    }

    out
}

fn render_slot_replacement(assignments: &SlotAssignments, slot: &SlotTag) -> String {
    let fragments = if slot.name == "default" {
        assignments.default.as_slice()
    } else {
        assignments
            .named
            .get(&slot.name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    };

    if fragments.is_empty() {
        return slot.fallback.clone().unwrap_or_default();
    }

    let mut out = String::new();
    for fragment in fragments {
        for binding in &fragment.let_bindings {
            if let Some(expr) = slot.props.get(binding) {
                out.push_str("{% let ");
                out.push_str(binding);
                out.push_str(" = ");
                out.push('(');
                out.push_str(expr);
                out.push_str(").clone()");
                out.push_str(" %}");
            }
        }
        out.push_str(&fragment.content);
    }
    out
}

fn extract_slot_name(attrs: &[HtmlAttr]) -> Option<String> {
    attrs
        .iter()
        .find(|a| a.name == "slot")
        .and_then(|a| a.value.clone())
        .map(|v| strip_wrapping_quotes(&v).to_string())
}

fn extract_slot_let_bindings(attrs: &[HtmlAttr]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|a| a.name.strip_prefix("let:").map(|s| s.to_string()))
        .collect()
}

fn render_html_attr(attr: &HtmlAttr) -> String {
    match (&attr.value, &attr.kind) {
        (None, _) => format!(" {}", attr.name),
        (Some(value), HtmlAttrKind::DoubleQuoted) => format!(" {}=\"{}\"", attr.name, value),
        (Some(value), HtmlAttrKind::SingleQuoted) => format!(" {}='{}'", attr.name, value),
        (Some(value), HtmlAttrKind::Braced) => format!(" {}={{{}}}", attr.name, value),
        (Some(value), HtmlAttrKind::Bare) => format!(" {}={}", attr.name, value),
    }
}

fn strip_wrapping_quotes(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}

fn is_pascal_case_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_uppercase() && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HtmlNode {
    name: String,
    attrs: Vec<HtmlAttr>,
    self_closing: bool,
    consumed: usize,
    inner: Option<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HtmlAttr {
    name: String,
    value: Option<String>,
    kind: HtmlAttrKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HtmlAttrKind {
    DoubleQuoted,
    SingleQuoted,
    Braced,
    Bare,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SlotTag {
    name: String,
    props: HashMap<String, String>,
    fallback: Option<String>,
    consumed: usize,
}

fn parse_html_node_at(input: &str, start: usize) -> Option<HtmlNode> {
    let open = parse_html_open_tag_at(input, start)?;
    if open.self_closing {
        return Some(HtmlNode {
            name: open.name,
            attrs: open.attrs,
            self_closing: true,
            consumed: open.consumed,
            inner: None,
        });
    }

    let open_end = start + open.consumed;
    let (close_start, close_end) = find_matching_html_close(input, open_end, &open.name)?;
    Some(HtmlNode {
        name: open.name,
        attrs: open.attrs,
        self_closing: false,
        consumed: close_end - start,
        inner: Some((open_end, close_start)),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HtmlOpenTag {
    name: String,
    attrs: Vec<HtmlAttr>,
    self_closing: bool,
    consumed: usize,
}

fn parse_html_open_tag_at(input: &str, start: usize) -> Option<HtmlOpenTag> {
    if !input[start..].starts_with('<') {
        return None;
    }

    let mut idx = start + 1;
    let first = input[idx..].chars().next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if input[idx..].starts_with('/')
        || input[idx..].starts_with('!')
        || input[idx..].starts_with('?')
    {
        return None;
    }

    let mut name_end = idx + first.len_utf8();
    while let Some(c) = input[name_end..].chars().next() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':') {
            name_end += c.len_utf8();
        } else {
            break;
        }
    }

    let name = input[idx..name_end].to_string();
    idx = name_end;

    let attrs_start = idx;
    let mut brace_depth = 0usize;
    let mut quote: Option<char> = None;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        let c_len = c.len_utf8();

        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            idx += c_len;
            continue;
        }

        match c {
            '"' | '\'' => {
                quote = Some(c);
                idx += c_len;
            }
            '{' => {
                brace_depth += 1;
                idx += c_len;
            }
            '}' => {
                if brace_depth == 0 {
                    return None;
                }
                brace_depth -= 1;
                idx += c_len;
            }
            '>' if brace_depth == 0 => {
                let attrs_src = &input[attrs_start..idx];
                let attrs = parse_html_attrs(attrs_src)?;
                let before = attrs_src.trim_end();
                let self_closing = before.ends_with('/');
                return Some(HtmlOpenTag {
                    name,
                    attrs,
                    self_closing,
                    consumed: idx + c_len - start,
                });
            }
            _ => idx += c_len,
        }
    }

    None
}

fn find_matching_html_close(input: &str, from: usize, name: &str) -> Option<(usize, usize)> {
    let mut idx = from;
    let mut depth = 1usize;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        if c != '<' {
            idx += c.len_utf8();
            continue;
        }

        if let Some(consumed) = parse_named_close_tag(input, idx, name) {
            depth -= 1;
            if depth == 0 {
                return Some((idx, idx + consumed));
            }
            idx += consumed;
            continue;
        }

        if let Some((consumed, self_closing)) = parse_named_open_tag(input, idx, name) {
            if !self_closing {
                depth += 1;
            }
            idx += consumed;
            continue;
        }

        idx += c.len_utf8();
    }

    None
}

fn parse_html_attrs(src: &str) -> Option<Vec<HtmlAttr>> {
    let mut attrs = Vec::new();
    let mut idx = 0usize;

    while idx < src.len() {
        idx = skip_ws(src, idx);
        if idx >= src.len() {
            break;
        }
        if src[idx..].starts_with('/') {
            // Self-closing marker in tags like `<slot />`
            break;
        }

        let (name, next_idx) = parse_html_attr_name(src, idx)?;
        idx = skip_ws(src, next_idx);

        if idx >= src.len() || !src[idx..].starts_with('=') {
            attrs.push(HtmlAttr {
                name,
                value: None,
                kind: HtmlAttrKind::Bare,
            });
            continue;
        }

        idx += 1;
        idx = skip_ws(src, idx);
        let (value, kind, consumed_to) = parse_html_attr_value(src, idx)?;
        attrs.push(HtmlAttr {
            name,
            value: Some(value),
            kind,
        });
        idx = consumed_to;
    }

    Some(attrs)
}

fn parse_html_attr_name(src: &str, start: usize) -> Option<(String, usize)> {
    let first = src[start..].chars().next()?;
    if !(first.is_ascii_alphabetic() || first == '_' || first == ':') {
        return None;
    }

    let mut idx = start + first.len_utf8();
    while let Some(c) = src[idx..].chars().next() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '-' | '.') {
            idx += c.len_utf8();
        } else {
            break;
        }
    }

    Some((src[start..idx].to_string(), idx))
}

fn parse_html_attr_value(src: &str, start: usize) -> Option<(String, HtmlAttrKind, usize)> {
    let first = src[start..].chars().next()?;
    match first {
        '"' => {
            let mut idx = start + 1;
            while idx < src.len() {
                let c = src[idx..].chars().next()?;
                if c == '"' {
                    return Some((
                        src[start + 1..idx].to_string(),
                        HtmlAttrKind::DoubleQuoted,
                        idx + 1,
                    ));
                }
                idx += c.len_utf8();
            }
            None
        }
        '\'' => {
            let mut idx = start + 1;
            while idx < src.len() {
                let c = src[idx..].chars().next()?;
                if c == '\'' {
                    return Some((
                        src[start + 1..idx].to_string(),
                        HtmlAttrKind::SingleQuoted,
                        idx + 1,
                    ));
                }
                idx += c.len_utf8();
            }
            None
        }
        '{' => {
            let (expr, end) = parse_braced_expr(src, start)?;
            Some((expr, HtmlAttrKind::Braced, end))
        }
        _ => {
            let mut idx = start;
            while let Some(c) = src[idx..].chars().next() {
                if c.is_whitespace() || c == '>' {
                    break;
                }
                idx += c.len_utf8();
            }
            Some((src[start..idx].to_string(), HtmlAttrKind::Bare, idx))
        }
    }
}

fn parse_slot_tag_at(input: &str, start: usize) -> Option<SlotTag> {
    let open = parse_html_open_tag_at(input, start)?;
    if !open.name.eq_ignore_ascii_case("slot") {
        return None;
    }

    let mut props = HashMap::new();
    let mut slot_name = "default".to_string();
    for attr in open.attrs {
        if attr.name == "name" {
            if let Some(value) = attr.value {
                slot_name = strip_wrapping_quotes(&value).to_string();
            }
            continue;
        }

        let Some(value) = attr.value else {
            continue;
        };
        let expr = match attr.kind {
            HtmlAttrKind::Braced => value,
            HtmlAttrKind::DoubleQuoted | HtmlAttrKind::SingleQuoted | HtmlAttrKind::Bare => {
                askama_expr_or_string_literal(&value)
            }
        };
        props.insert(attr.name, expr);
    }

    if open.self_closing {
        return Some(SlotTag {
            name: slot_name,
            props,
            fallback: None,
            consumed: open.consumed,
        });
    }

    let open_end = start + open.consumed;
    let (close_start, close_end) = find_matching_html_close(input, open_end, "slot")?;
    Some(SlotTag {
        name: slot_name,
        props,
        fallback: Some(input[open_end..close_start].to_string()),
        consumed: close_end - start,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedComponentInvocation {
    name: String,
    attrs: Vec<(String, String)>,
    inner: Option<String>,
    consumed: usize,
}

fn parse_component_invocation(input: &str) -> Option<ParsedComponentInvocation> {
    let mut idx = 0usize;
    idx += consume_char(input, idx, '<')?;

    let first = input[idx..].chars().next()?;
    if !first.is_ascii_uppercase() {
        return None;
    }

    let mut name_end = idx + first.len_utf8();
    while let Some(c) = input[name_end..].chars().next() {
        if c.is_ascii_alphanumeric() || c == '_' {
            name_end += c.len_utf8();
        } else {
            break;
        }
    }

    let name = input[idx..name_end].to_string();
    idx = name_end;

    let attrs_start = idx;
    let mut brace_depth = 0usize;
    let mut quote: Option<char> = None;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        let c_len = c.len_utf8();

        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            idx += c_len;
            continue;
        }

        match c {
            '"' | '\'' => {
                quote = Some(c);
                idx += c_len;
            }
            '{' => {
                brace_depth += 1;
                idx += c_len;
            }
            '}' => {
                if brace_depth == 0 {
                    return None;
                }
                brace_depth -= 1;
                idx += c_len;
            }
            '/' if brace_depth == 0 && input[idx..].starts_with("/>") => {
                let attrs = parse_attributes(&input[attrs_start..idx])?;
                return Some(ParsedComponentInvocation {
                    name,
                    attrs,
                    inner: None,
                    consumed: idx + 2,
                });
            }
            '>' => {
                let attrs = parse_attributes(&input[attrs_start..idx])?;
                let open_end = idx + c_len;
                let (inner_len, close_len) =
                    find_matching_component_close(&input[open_end..], &name)?;
                let inner = input[open_end..open_end + inner_len].to_string();
                return Some(ParsedComponentInvocation {
                    name,
                    attrs,
                    inner: Some(inner),
                    consumed: open_end + inner_len + close_len,
                });
            }
            _ => idx += c_len,
        }
    }

    None
}

fn find_matching_component_close(input: &str, name: &str) -> Option<(usize, usize)> {
    let mut idx = 0usize;
    let mut depth = 1usize;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        if c != '<' {
            idx += c.len_utf8();
            continue;
        }

        if let Some(consumed) = parse_named_close_tag(input, idx, name) {
            depth -= 1;
            if depth == 0 {
                return Some((idx, consumed));
            }
            idx += consumed;
            continue;
        }

        if let Some((consumed, self_closing)) = parse_named_open_tag(input, idx, name) {
            if !self_closing {
                depth += 1;
            }
            idx += consumed;
            continue;
        }

        idx += c.len_utf8();
    }

    None
}

fn parse_named_open_tag(input: &str, start: usize, name: &str) -> Option<(usize, bool)> {
    if !input[start..].starts_with('<') {
        return None;
    }

    let mut idx = start + 1;
    if input[idx..].starts_with('/') {
        return None;
    }
    if !input[idx..].starts_with(name) {
        return None;
    }
    idx += name.len();

    let boundary = input[idx..].chars().next()?;
    if !(boundary.is_whitespace() || boundary == '/' || boundary == '>') {
        return None;
    }

    let attrs_start = idx;
    let mut brace_depth = 0usize;
    let mut quote: Option<char> = None;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        let c_len = c.len_utf8();

        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            idx += c_len;
            continue;
        }

        match c {
            '"' | '\'' => {
                quote = Some(c);
                idx += c_len;
            }
            '{' => {
                brace_depth += 1;
                idx += c_len;
            }
            '}' => {
                if brace_depth == 0 {
                    return None;
                }
                brace_depth -= 1;
                idx += c_len;
            }
            '>' if brace_depth == 0 => {
                let before = input[attrs_start..idx].trim_end();
                let self_closing = before.ends_with('/');
                return Some((idx + c_len - start, self_closing));
            }
            _ => idx += c_len,
        }
    }

    None
}

fn parse_named_close_tag(input: &str, start: usize, name: &str) -> Option<usize> {
    if !input[start..].starts_with("</") {
        return None;
    }

    let mut idx = start + 2;
    if !input[idx..].starts_with(name) {
        return None;
    }
    idx += name.len();

    let boundary = input[idx..].chars().next()?;
    if !(boundary.is_whitespace() || boundary == '>') {
        return None;
    }

    idx = skip_ws(input, idx);
    if !input[idx..].starts_with('>') {
        return None;
    }

    Some(idx + 1 - start)
}

fn parse_attributes(src: &str) -> Option<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut idx = 0usize;

    while idx < src.len() {
        idx = skip_ws(src, idx);
        if idx >= src.len() {
            break;
        }

        let (name, next_idx) = parse_attr_name(src, idx)?;
        idx = skip_ws(src, next_idx);

        if idx >= src.len() || !src[idx..].starts_with('=') {
            out.push((name, "true".to_string()));
            continue;
        }

        idx += 1;
        idx = skip_ws(src, idx);
        let (expr, consumed_to) = parse_attr_value(src, idx)?;
        out.push((name, expr));
        idx = consumed_to;
    }

    Some(out)
}

fn parse_attr_name(src: &str, start: usize) -> Option<(String, usize)> {
    let first = src[start..].chars().next()?;
    if !is_rust_ident_start(first) {
        return None;
    }

    let mut idx = start + first.len_utf8();
    while let Some(c) = src[idx..].chars().next() {
        if is_rust_ident_continue(c) {
            idx += c.len_utf8();
        } else {
            break;
        }
    }

    Some((src[start..idx].to_string(), idx))
}

fn parse_attr_value(src: &str, start: usize) -> Option<(String, usize)> {
    let first = src[start..].chars().next()?;
    match first {
        '{' => parse_braced_expr(src, start),
        '"' | '\'' => parse_quoted_expr(src, start, first),
        _ => {
            let mut idx = start;
            while let Some(c) = src[idx..].chars().next() {
                if c.is_whitespace() {
                    break;
                }
                idx += c.len_utf8();
            }
            Some((src[start..idx].to_string(), idx))
        }
    }
}

fn parse_braced_expr(src: &str, start: usize) -> Option<(String, usize)> {
    let mut idx = start + 1;
    let mut depth = 1usize;

    while idx < src.len() {
        let c = src[idx..].chars().next()?;
        let c_len = c.len_utf8();
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let expr = src[start + 1..idx].trim().to_string();
                    return Some((expr, idx + c_len));
                }
            }
            _ => {}
        }
        idx += c_len;
    }

    None
}

fn parse_quoted_expr(src: &str, start: usize, quote: char) -> Option<(String, usize)> {
    let mut idx = start + quote.len_utf8();
    while idx < src.len() {
        let c = src[idx..].chars().next()?;
        let c_len = c.len_utf8();
        if c == quote {
            let raw = &src[start + quote.len_utf8()..idx];
            let expr = askama_expr_or_string_literal(raw);
            return Some((expr, idx + c_len));
        }
        idx += c_len;
    }
    None
}

fn askama_expr_or_string_literal(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(inner) = trimmed
        .strip_prefix("{{")
        .and_then(|v| v.strip_suffix("}}"))
    {
        return inner.trim().to_string();
    }
    format!("{raw:?}")
}

fn skip_ws(src: &str, mut idx: usize) -> usize {
    while idx < src.len() {
        let Some(c) = src[idx..].chars().next() else {
            break;
        };
        if c.is_whitespace() {
            idx += c.len_utf8();
        } else {
            break;
        }
    }
    idx
}

fn consume_char(src: &str, idx: usize, expected: char) -> Option<usize> {
    let c = src[idx..].chars().next()?;
    if c == expected {
        Some(c.len_utf8())
    } else {
        None
    }
}

fn is_rust_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

fn is_rust_ident_continue(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
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
<html><body><slot /></body></html>"#,
        );

        let result = compile_to_out_dir(&src, &out).expect("pipeline should compile");

        assert_eq!(result.preprocessed_files.len(), 3);
        assert!(result.generated_routes_file.exists());
        assert!(result.generated_templates_file.exists());
        assert!(result.generated_routes.iter().any(|r| r.pattern == "/"));
        assert!(
            result
                .generated_templates
                .iter()
                .any(|t| t.render_symbol == "render_page_index")
        );

        let page_template = out.join("pilcrow_templates/pages/index.html");
        let page_rendered = fs::read_to_string(page_template).expect("read transpiled page");
        assert!(page_rendered.contains("<html><body>"));
        assert!(page_rendered.contains("<article>{{ title }}</article>"));
        assert!(!page_rendered.contains("<Layout"));
        assert!(!page_rendered.contains("{{ Layout {"));
        assert!(!page_rendered.contains("{{ Card {"));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_expands_named_slots() {
        let root = mk_temp_root("compile_named_slots");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/index.html"),
            r#"---
pub struct Props {}
---
<Layout>
    <h1 slot="header">Top</h1>
    <p>Body</p>
</Layout>"#,
        );
        write_file(
            &src.join("layouts/Layout.html"),
            r#"---
pub struct Props {}
---
<header><slot name="header" /></header>
<main><slot /></main>"#,
        );

        let _result = compile_to_out_dir(&src, &out).expect("pipeline should compile");
        let page_template = out.join("pilcrow_templates/pages/index.html");
        let page_rendered = fs::read_to_string(page_template).expect("read transpiled page");

        assert!(page_rendered.contains("<header><h1>Top</h1></header>"));
        assert!(page_rendered.contains("<main>"));
        assert!(page_rendered.contains("<p>Body</p>"));
        assert!(!page_rendered.contains("slot=\"header\""));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_expands_slot_props_via_let_bindings() {
        let root = mk_temp_root("compile_slot_props");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/index.html"),
            r#"---
pub struct Props {
    pub title: String,
}
---
<List title={title}>
    <li slot="item" let:item>{{ item }}</li>
</List>"#,
        );
        write_file(
            &src.join("components/List.html"),
            r#"---
pub struct Props {
    pub title: String,
}
---
<ul><slot name="item" item={title} /></ul>"#,
        );

        let _result = compile_to_out_dir(&src, &out).expect("pipeline should compile");
        let page_template = out.join("pilcrow_templates/pages/index.html");
        let page_rendered = fs::read_to_string(page_template).expect("read transpiled page");

        assert!(page_rendered.contains("{% let item = (title).clone() %}<li>{{ item }}</li>"));
        assert!(page_rendered.contains("<ul>"));
        assert!(!page_rendered.contains("let:item"));
        assert!(!page_rendered.contains("slot=\"item\""));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_keeps_unknown_component_as_call_syntax() {
        let root = mk_temp_root("compile_unknown_component");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/index.html"),
            r#"---
pub struct Props {}
---
<UnknownWidget title={title} />"#,
        );

        let _result = compile_to_out_dir(&src, &out).expect("pipeline should compile");
        let page_template = out.join("pilcrow_templates/pages/index.html");
        let page_rendered = fs::read_to_string(page_template).expect("read transpiled page");
        assert_eq!(page_rendered, "{{ UnknownWidget { title: title }|safe }}");

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
