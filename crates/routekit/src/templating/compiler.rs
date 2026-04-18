use std::fmt;

/// Split result for a Pilcrow `.html` file that uses fenced Rust + template sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlModuleParts {
    pub rust: String,
    pub template: String,
}

/// Parse failures for `.html` module splitting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlModuleParseError {
    MissingFence,
    EmptyTemplate,
}

impl fmt::Display for HtmlModuleParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFence => {
                write!(
                    f,
                    "expected `---` fenced file format with Rust block and template block"
                )
            }
            Self::EmptyTemplate => write!(f, "template section after second fence is empty"),
        }
    }
}

impl std::error::Error for HtmlModuleParseError {}

/// Splits a `.html` source string into Rust frontmatter and template body.
///
/// Expected format:
///
/// ```text
/// ---
/// // Rust code...
/// ---
/// <h1>Template</h1>
/// ```
pub fn split_html_module(input: &str) -> Result<HtmlModuleParts, HtmlModuleParseError> {
    // A file with no leading `---` fence is treated as pure template body with
    // empty Rust frontmatter. This lets purely-static pages and components omit
    // the ceremony of declaring an empty `pub struct Props {}`.
    if !input.trim_start().starts_with("---") {
        let template = input.trim();
        if template.is_empty() {
            return Err(HtmlModuleParseError::EmptyTemplate);
        }
        return Ok(HtmlModuleParts {
            rust: String::new(),
            template: template.to_string(),
        });
    }

    let mut parts = input.splitn(3, "---");
    let leading = parts.next().unwrap_or_default();
    let rust = parts.next().ok_or(HtmlModuleParseError::MissingFence)?;
    let template = parts.next().ok_or(HtmlModuleParseError::MissingFence)?;

    if !leading.trim().is_empty() {
        return Err(HtmlModuleParseError::MissingFence);
    }

    let template = template.trim();
    if template.is_empty() {
        return Err(HtmlModuleParseError::EmptyTemplate);
    }

    Ok(HtmlModuleParts {
        rust: rust.trim().to_string(),
        template: template.to_string(),
    })
}

/// Splits and transpiles a Pilcrow `.html` module.
///
/// This performs:
/// 1. `---` fence splitting
/// 2. component tag transpilation in the template section
/// 3. form progressive-enhancement injection
#[allow(dead_code)]
pub fn transpile_html_module(input: &str) -> Result<HtmlModuleParts, HtmlModuleParseError> {
    let mut parts = split_html_module(input)?;
    parts.template = transpile_component_tags(&parts.template);
    parts.template = inject_form_method_attrs(&parts.template);
    Ok(parts)
}

// ── HTTP Verb Attributes ──────────────────────────────────────

/// Maps silcrow verb attributes to their native HTTP method strings.
/// HTML forms only support GET/POST natively; all mutation verbs map to POST.
const VERB_ATTRS: &[(&str, &str)] = &[
    ("s-post",   "post"),
    ("s-put",    "post"),
    ("s-patch",  "post"),
    ("s-delete", "post"),
];

/// Injects `method` and `action` attributes into `<form>` elements that carry
/// a silcrow verb attribute (`s-post`, `s-put`, `s-patch`, `s-delete`).
///
/// This enables progressive enhancement: a form with `s-post="?action=create"`
/// works as a native HTML form submit when JS is unavailable, and silcrow.js
/// intercepts it when JS is present.
///
/// Rules:
/// - `s-post`   → `method="post"`
/// - `s-put|s-patch|s-delete` → `method="post"` (HTML only supports GET/POST natively)
/// - If `method` already exists on the element: not overwritten.
/// - If `action` already exists on the element: not overwritten.
/// - `s-get` on forms is left alone — GET is already the HTML default.
pub fn inject_form_method_attrs(template: &str) -> String {
    let mut output = String::with_capacity(template.len() + 64);
    let mut i = 0;

    while i < template.len() {
        // Fast path: look for the literal substring "<form"
        if template[i..].starts_with("<form") {
            let after = i + 5;
            let next_char = template[after..].chars().next();
            // Must be followed by whitespace or '>' to be a real <form> tag
            if matches!(next_char, Some(c) if c.is_whitespace() || c == '>') {
                if let Some((transformed, consumed)) = try_inject_form_tag(&template[i..]) {
                    output.push_str(&transformed);
                    i += consumed;
                    continue;
                }
            }
        }

        let c = template[i..].chars().next().unwrap();
        output.push(c);
        i += c.len_utf8();
    }

    output
}

/// Try to transform a `<form ...>` opening tag by injecting progressive-enhancement
/// attributes. Returns `None` if the tag has no silcrow verb attribute (no-op).
fn try_inject_form_tag(input: &str) -> Option<(String, usize)> {
    debug_assert!(input.starts_with("<form"));

    // Collect the raw text of the opening tag's attribute section (between "<form" and ">").
    let mut idx = 5; // skip "<form"
    let mut raw_attrs = String::new();
    let mut quote: Option<char> = None;
    let mut brace_depth: usize = 0;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        let c_len = c.len_utf8();

        if let Some(q) = quote {
            raw_attrs.push(c);
            if c == q { quote = None; }
            idx += c_len;
            continue;
        }

        match c {
            '"' | '\'' => { quote = Some(c); raw_attrs.push(c); idx += c_len; }
            '{' => { brace_depth += 1; raw_attrs.push(c); idx += c_len; }
            '}' => {
                brace_depth = brace_depth.saturating_sub(1);
                raw_attrs.push(c);
                idx += c_len;
            }
            '>' if brace_depth == 0 => {
                // Found the end of the opening tag.
                let tag_end = idx + 1; // include '>'

                // Does this form have a silcrow mutation verb attribute?
                let verb_match = VERB_ATTRS.iter().find_map(|&(attr, method)| {
                    html_attr_value(&raw_attrs, attr).map(|url| (method, url))
                });

                let (http_method, url) = verb_match?; // no verb → return None (no-op)

                let has_method = html_has_attr(&raw_attrs, "method");
                let has_action = html_has_attr(&raw_attrs, "action");

                if has_method && has_action {
                    return None; // nothing to inject
                }

                let mut inject = String::new();
                if !has_method {
                    inject.push_str(&format!(" method=\"{http_method}\""));
                }
                if !has_action {
                    let safe_url = url.replace('"', "&quot;");
                    inject.push_str(&format!(" action=\"{safe_url}\""));
                }

                let transformed = format!("<form{raw_attrs}{inject}>");
                return Some((transformed, tag_end));
            }
            _ => { raw_attrs.push(c); idx += c_len; }
        }
    }

    None
}

/// Extract the value of a named HTML attribute from a raw attribute string.
/// Handles `name="value"`, `name='value'`. Returns `None` if not found.
fn html_attr_value(attrs: &str, name: &str) -> Option<String> {
    let mut i = 0;
    while i < attrs.len() {
        i = skip_ws(attrs, i);
        if i >= attrs.len() { break; }

        let (attr_name, next) = scan_html_attr_name(attrs, i);
        i = skip_ws(attrs, next);

        if i < attrs.len() && attrs[i..].starts_with('=') {
            i += 1; // skip '='
            i = skip_ws(attrs, i);
            if let Some((val, end)) = scan_html_attr_val(attrs, i) {
                if attr_name == name {
                    return Some(val);
                }
                i = end;
            } else {
                i = next; // can't parse value; skip
            }
        } else {
            // Bare attribute (no value)
            if attr_name == name { return Some(String::new()); }
        }
    }
    None
}

/// Return true if a named attribute is present in the raw attribute string.
fn html_has_attr(attrs: &str, name: &str) -> bool {
    html_attr_value(attrs, name).is_some()
}

/// Scan an HTML attribute name (letters, digits, hyphens, colons, underscores, dots).
/// Returns `(name, end_index)`.
fn scan_html_attr_name(src: &str, start: usize) -> (String, usize) {
    let mut idx = start;
    while idx < src.len() {
        let c = src[idx..].chars().next().unwrap_or('\0');
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.') {
            idx += c.len_utf8();
        } else {
            break;
        }
    }
    (src[start..idx].to_string(), idx)
}

/// Scan a quoted (`"..."` or `'...'`) or braced (`{...}`) HTML attribute value.
/// Returns `(value_text, end_index)`.
fn scan_html_attr_val(src: &str, start: usize) -> Option<(String, usize)> {
    let first = src[start..].chars().next()?;
    match first {
        '"' | '\'' => {
            let mut idx = start + 1;
            while idx < src.len() {
                let c = src[idx..].chars().next()?;
                if c == first {
                    let val = src[start + 1..idx].to_string();
                    return Some((val, idx + 1));
                }
                idx += c.len_utf8();
            }
            None
        }
        '{' => {
            let mut depth = 1usize;
            let mut idx = start + 1;
            while idx < src.len() {
                let c = src[idx..].chars().next()?;
                match c { '{' => depth += 1, '}' => { depth -= 1; if depth == 0 { return Some((src[start + 1..idx].to_string(), idx + 1)); } } _ => {} }
                idx += c.len_utf8();
            }
            None
        }
        _ => None,
    }
}

/// Transpiles PascalCase component tags into Askama expressions.
///
/// Example:
/// `<Card title={item.title} />`
/// becomes
/// `{{ Card { title: item.title }|safe }}`
///
/// For paired component tags, inner content is captured into a synthetic
/// `children` field:
/// `<Layout title={title}>...</Layout>`
/// becomes
/// `{{ Layout { title: title, children: r#"..."# }|safe }}`
pub fn transpile_component_tags(template: &str) -> String {
    let mut output = String::with_capacity(template.len());
    let mut i = 0usize;

    while i < template.len() {
        let Some(ch) = template[i..].chars().next() else {
            break;
        };

        if ch == '<'
            && let Some((replacement, consumed)) = parse_component_tag(&template[i..])
        {
            output.push_str(&replacement);
            i += consumed;
            continue;
        }

        output.push(ch);
        i += ch.len_utf8();
    }

    output
}

fn parse_component_tag(input: &str) -> Option<(String, usize)> {
    let mut idx = 0usize;

    // Must start with "<"
    idx += consume_char(input, idx, '<')?;

    // Component name: PascalCase + [a-zA-Z0-9_]
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

    let name = &input[idx..name_end];
    idx = name_end;

    // Parse opening tag until "/>" or ">", honoring nested braces and quoted strings.
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
                let attrs_src = &input[attrs_start..idx];
                let attrs = parse_attributes(attrs_src)?;
                let rendered = render_component_call(name, &attrs);
                return Some((rendered, idx + 2));
            }
            '>' => {
                let attrs_src = &input[attrs_start..idx];
                let attrs = parse_attributes(attrs_src)?;
                let open_end = idx + c_len;
                let (inner_len, close_len) =
                    find_matching_component_close(&input[open_end..], name)?;
                let inner = &input[open_end..open_end + inner_len];
                let consumed = open_end + inner_len + close_len;

                let inner_transpiled = transpile_component_tags(inner);
                let rendered =
                    render_component_call_with_children(name, &attrs, &inner_transpiled)?;
                return Some((rendered, consumed));
            }
            _ => {
                idx += c_len;
            }
        }
    }

    None
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
            // Bare attrs (e.g. disabled) become booleans.
            out.push((name, "true".to_string()));
            continue;
        }
        idx += 1; // '='
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
            // Unquoted token
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
    let mut idx = start + 1; // skip opening {
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

fn render_component_call(name: &str, attrs: &[(String, String)]) -> String {
    if attrs.is_empty() {
        return format!("{{{{ {name} {{}}|safe }}}}");
    }

    let body = attrs
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join(", ");

    format!("{{{{ {name} {{ {body} }}|safe }}}}")
}

fn render_component_call_with_children(
    name: &str,
    attrs: &[(String, String)],
    inner: &str,
) -> Option<String> {
    if inner.trim().is_empty() {
        return Some(render_component_call(name, attrs));
    }

    if attrs.iter().any(|(k, _)| k == "children") {
        return None;
    }

    let mut all = attrs.to_vec();
    all.push(("children".to_string(), rust_raw_string_literal(inner)));
    Some(render_component_call(name, &all))
}

fn rust_raw_string_literal(value: &str) -> String {
    for hashes_count in 0..=32usize {
        let hashes = "#".repeat(hashes_count);
        let terminator = format!("\"{hashes}");
        if !value.contains(&terminator) {
            return format!("r{hashes}\"{value}\"{hashes}");
        }
    }
    format!("{value:?}")
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
            _ => {
                idx += c_len;
            }
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

fn is_rust_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

fn is_rust_ident_continue(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

fn consume_char(src: &str, idx: usize, expected: char) -> Option<usize> {
    let c = src[idx..].chars().next()?;
    if c == expected {
        Some(c.len_utf8())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_html_module_success() {
        let source = r#"---
use crate::models::Post;

pub struct Props {
    pub title: String,
}
---
<h1>{{ title }}</h1>"#;

        let parts = split_html_module(source).expect("expected valid split");
        assert!(parts.rust.contains("pub struct Props"));
        assert_eq!(parts.template, "<h1>{{ title }}</h1>");
    }

    #[test]
    fn split_html_module_allows_missing_fence_as_static_template() {
        let parts = split_html_module("<h1>Only template</h1>").expect("expected valid split");
        assert_eq!(parts.rust, "");
        assert_eq!(parts.template, "<h1>Only template</h1>");
    }

    #[test]
    fn split_html_module_rejects_empty_template() {
        let err = split_html_module("---\nlet x = 1;\n---\n\n").expect_err("expected an error");
        assert_eq!(err, HtmlModuleParseError::EmptyTemplate);
    }

    #[test]
    fn transpile_component_tag_basic() {
        let input = r#"<Card title={item.title} active={item.active} />"#;
        let output = transpile_component_tags(input);
        assert_eq!(
            output,
            "{{ Card { title: item.title, active: item.active }|safe }}"
        );
    }

    #[test]
    fn transpile_component_tag_askama_quoted_expr() {
        let input = r#"<Card title="{{ item.title }}" />"#;
        let output = transpile_component_tags(input);
        assert_eq!(output, "{{ Card { title: item.title }|safe }}");
    }

    #[test]
    fn transpile_component_tag_string_literal() {
        let input = r#"<Badge label="new" />"#;
        let output = transpile_component_tags(input);
        assert_eq!(output, "{{ Badge { label: \"new\" }|safe }}");
    }

    #[test]
    fn transpile_component_tag_without_props() {
        let input = "<Footer />";
        let output = transpile_component_tags(input);
        assert_eq!(output, "{{ Footer {}|safe }}");
    }

    #[test]
    fn transpile_component_with_paired_children() {
        let input = "<Layout title={title}><h1>Hello</h1></Layout>";
        let output = transpile_component_tags(input);
        assert_eq!(
            output,
            "{{ Layout { title: title, children: r\"<h1>Hello</h1>\" }|safe }}"
        );
    }

    #[test]
    fn transpile_component_with_nested_children_components() {
        let input = "<Layout title={title}><Card title={title} /></Layout>";
        let output = transpile_component_tags(input);
        assert_eq!(
            output,
            "{{ Layout { title: title, children: r\"{{ Card { title: title }|safe }}\" }|safe }}"
        );
    }

    #[test]
    fn transpile_component_with_empty_paired_body() {
        let input = "<Footer></Footer>";
        let output = transpile_component_tags(input);
        assert_eq!(output, "{{ Footer {}|safe }}");
    }

    #[test]
    fn transpile_component_handles_nested_same_name_tags() {
        let input = "<Box><Box /></Box>";
        let output = transpile_component_tags(input);
        assert_eq!(
            output,
            "{{ Box { children: r\"{{ Box {}|safe }}\" }|safe }}"
        );
    }

    #[test]
    fn transpile_ignores_lowercase_html_tags() {
        let input = r#"<div class="x"><span>Hi</span></div>"#;
        let output = transpile_component_tags(input);
        assert_eq!(output, input);
    }

    #[test]
    fn transpile_ignores_invalid_component_attrs() {
        let input = r#"<Card s-key=".id" title=".title" />"#;
        let output = transpile_component_tags(input);
        assert_eq!(output, input);
    }

    // ── inject_form_method_attrs ──────────────────────────────

    #[test]
    fn inject_form_injects_method_and_action_for_s_post() {
        let input = r##"<form s-post="?action=create" s-target="#f">"##;
        let output = inject_form_method_attrs(input);
        assert!(output.contains(r#"method="post""#), "should inject method: {output}");
        assert!(output.contains(r#"action="?action=create""#), "should inject action: {output}");
    }

    #[test]
    fn inject_form_does_not_overwrite_existing_method() {
        let input = r#"<form s-post="?action=create" method="POST">"#;
        let output = inject_form_method_attrs(input);
        // Only one method= present
        assert_eq!(output.matches("method=").count(), 1);
    }

    #[test]
    fn inject_form_does_not_overwrite_existing_action() {
        let input = r#"<form s-post="?action=create" action="/custom">"#;
        let output = inject_form_method_attrs(input);
        assert!(output.contains(r#"action="/custom""#));
        assert!(!output.contains(r#"action="?action=create""#));
    }

    #[test]
    fn inject_form_ignores_s_get() {
        let input = r#"<form s-get="/search">"#;
        let output = inject_form_method_attrs(input);
        assert_eq!(input, output, "s-get should not be modified");
    }

    #[test]
    fn inject_form_handles_s_delete() {
        let input = r#"<form s-delete="/items/1">"#;
        let output = inject_form_method_attrs(input);
        assert!(output.contains(r#"method="post""#));
        assert!(output.contains(r#"action="/items/1""#));
    }

    #[test]
    fn inject_form_ignores_non_form_elements() {
        let input = r#"<div s-post="?action=create"></div>"#;
        let output = inject_form_method_attrs(input);
        assert_eq!(input, output, "non-form elements should not be modified");
    }

    #[test]
    fn inject_form_leaves_forms_without_verb_attrs_untouched() {
        let input = r#"<form id="search" class="form">"#;
        let output = inject_form_method_attrs(input);
        assert_eq!(input, output);
    }

    #[test]
    fn inject_form_handles_multiple_forms() {
        let input = r#"<form s-post="?action=login"><form s-post="?action=signup">"#;
        let output = inject_form_method_attrs(input);
        assert_eq!(output.matches(r#"method="post""#).count(), 2);
        assert!(output.contains(r#"action="?action=login""#));
        assert!(output.contains(r#"action="?action=signup""#));
    }

    #[test]
    fn transpile_ignores_mismatched_paired_tags() {
        let input = "<Layout><Card /></Layot>";
        let output = transpile_component_tags(input);
        assert_eq!(output, "<Layout>{{ Card {}|safe }}</Layot>");
    }

    #[test]
    fn transpile_handles_multiple_components() {
        let input = r#"
{% for item in items %}
    <Card title={item.title} />
    <Badge label="new" />
{% endfor %}
"#;

        let output = transpile_component_tags(input);
        assert!(output.contains("{{ Card { title: item.title }|safe }}"));
        assert!(output.contains("{{ Badge { label: \"new\" }|safe }}"));
    }

    #[test]
    fn transpile_html_module_runs_split_and_template_transform() {
        let source = r#"---
pub struct Props { pub title: String }
---
<Card title={title} />"#;

        let parts = transpile_html_module(source).expect("should parse/transpile");
        assert!(parts.rust.contains("pub struct Props"));
        assert_eq!(parts.template, "{{ Card { title: title }|safe }}");
    }
}
