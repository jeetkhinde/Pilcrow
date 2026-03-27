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
pub fn transpile_html_module(input: &str) -> Result<HtmlModuleParts, HtmlModuleParseError> {
    let mut parts = split_html_module(input)?;
    parts.template = transpile_component_tags(&parts.template);
    Ok(parts)
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

        if ch == '<' {
            if let Some((replacement, consumed)) = parse_component_tag(&template[i..]) {
                output.push_str(&replacement);
                i += consumed;
                continue;
            }
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
    fn split_html_module_rejects_missing_fence() {
        let err = split_html_module("<h1>Only template</h1>").expect_err("expected an error");
        assert_eq!(err, HtmlModuleParseError::MissingFence);
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
