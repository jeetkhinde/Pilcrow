use serde::Serialize;
use syn::{FnArg, Item, ReturnType, Type, Visibility};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub rule_id: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub findings: Vec<Finding>,
}

pub fn validate_implementation(
    code: &str,
    path: Option<&str>,
    kind: Option<&str>,
) -> ValidationReport {
    let mut findings = Vec::new();
    let is_rust = kind == Some("rust")
        || path
            .map(|path| path.ends_with(".rs"))
            .unwrap_or_else(|| looks_like_rust(code));
    let is_html = kind == Some("html")
        || path
            .map(|path| path.ends_with(".html"))
            .unwrap_or_else(|| !is_rust);

    if is_rust {
        validate_rust(code, path, kind, &mut findings);
    }
    if is_html {
        validate_html(code, path, &mut findings);
    }

    let valid = !findings
        .iter()
        .any(|finding| finding.severity == Severity::Error);
    ValidationReport { valid, findings }
}

fn validate_rust(code: &str, path: Option<&str>, kind: Option<&str>, findings: &mut Vec<Finding>) {
    // Scan text lines first for quick checks with line numbers
    for (idx, line) in code.lines().enumerate() {
        let lnum = idx + 1;
        let line_lower = line.to_ascii_lowercase();

        for directive in ["s-boost", "s-target", "s-swap", "s-trigger", "client:", "<Island"] {
            if line.contains(directive) {
                findings.push(finding_with_line(
                    Severity::Error,
                    "pilcrow-boundary-silcrow-in-rust",
                    format!("Silcrow/client directive `{directive}` belongs in HTML templates, not Rust code."),
                    path,
                    Some(lnum),
                    Some("crates/runtime/assets/silcrow.js — directives are HTML-only"),
                    Some("Move the directive to the paired .html template."),
                ));
            }
        }

        // Warn on direct std::env::var access when env-config may be appropriate
        if line.contains("std::env::var(") || line.contains("env::var(") {
            if !line_lower.contains("//") {
                findings.push(finding_with_line(
                    Severity::Info,
                    "pilcrow-env-direct-access",
                    "Direct env::var() usage detected. Consider typed env config via Pilcrow.toml [env] and generated env::Public/Private helpers.".to_string(),
                    path,
                    Some(lnum),
                    Some("crates/routekit/src/templating/build_config.rs"),
                    Some("Add env vars to Pilcrow.toml [env] and use generated env::Private::load()."),
                ));
            }
        }

        // Detect SSG/ISR planned constants
        for planned_const in ["PRERENDER", "REVALIDATE", "GENERATE_STATIC_PARAMS"] {
            if line.contains(planned_const) && line.contains("const") {
                findings.push(finding_with_line(
                    Severity::Error,
                    "pilcrow-planned-static-output",
                    format!("`{planned_const}` is a planned feature (SSG/ISR) and is not currently supported."),
                    path,
                    Some(lnum),
                    Some("registry.toml: feature ssg, feature incremental-ssr"),
                    Some("Remove this constant. Use server-side caching or Deferred<T> for deferred loading."),
                ));
            }
        }
    }

    if kind != Some("api")
        && (code.contains("Router::new().route(") || code.contains(".route("))
        && !code.contains("pub fn router()")
    {
        findings.push(finding(
            Severity::Warning,
            "pilcrow-no-manual-route-registration",
            "Page routes should be discovered by Pilcrow routekit instead of manually registered."
                .to_string(),
            path,
            Some("crates/routekit/src/templating/codegen/app_module.rs"),
            Some("Use pilcrow_web::include_generated_app!() and let routekit register pages."),
        ));
    }

    let parsed = match syn::parse_file(code) {
        Ok(file) => file,
        Err(error) => {
            findings.push(finding(
                Severity::Error,
                "rust-parse-error",
                format!("Rust snippet does not parse: {error}"),
                path,
                None,
                None,
            ));
            return;
        }
    };

    let is_layout = path.map(|p| p.contains("_layout")).unwrap_or(false);
    let is_ui_component = path.map(|p| p.contains("/ui/") || p.contains("\\ui\\")).unwrap_or(false);

    for item in &parsed.items {
        match item {
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if name == "load" {
                    if function.sig.asyncness.is_none() {
                        findings.push(finding(
                            Severity::Error,
                            "pilcrow-load-async",
                            "load must be declared async.".to_string(),
                            path,
                            Some("crates/routekit/src/templating/codegen/instrument.rs"),
                            Some("Use `pub async fn load(req: Req) -> AppResult<Props>`."),
                        ));
                    }
                    if !returns_named_type(&function.sig.output, "AppResult") {
                        findings.push(finding(
                            Severity::Error,
                            "pilcrow-load-return",
                            "load should return AppResult<Props>.".to_string(),
                            path,
                            Some("crates/routekit/src/templating/codegen/instrument.rs"),
                            Some("Use `-> AppResult<Props>` and return `Ok(Props { ... })`."),
                        ));
                    }
                    if !wants_type(&function.sig.inputs, "Req") {
                        findings.push(finding(
                            Severity::Warning,
                            "pilcrow-load-req",
                            "load should accept a Req argument for current code-behind conventions."
                                .to_string(),
                            path,
                            Some("crates/routekit/src/templating/codegen/instrument.rs"),
                            Some("Use `pub async fn load(req: Req) -> AppResult<Props>` or `_req: Req` if unused."),
                        ));
                    }
                } else if matches!(function.vis, Visibility::Public(_))
                    && wants_type(&function.sig.inputs, "Req")
                {
                    // Layout and UI components cannot define actions at all
                    if is_layout {
                        findings.push(finding(
                            Severity::Error,
                            "pilcrow-layout-no-actions",
                            format!("Layouts cannot define named action `{name}`. Only load() is permitted in layout code-behind."),
                            path,
                            Some("crates/routekit/src/templating/codegen/instrument.rs"),
                            Some("Remove the action fn from _layout.rs. Move it to the page code-behind instead."),
                        ));
                    } else if is_ui_component {
                        findings.push(finding(
                            Severity::Error,
                            "pilcrow-component-no-actions",
                            format!("UI components cannot define named action `{name}`. Actions belong in page code-behind files."),
                            path,
                            Some("crates/routekit/src/templating/codegen/instrument.rs"),
                            Some("Move the action to the page .rs file that uses this component."),
                        ));
                    } else if !returns_named_type(&function.sig.output, "ActionResult") {
                        findings.push(finding(
                            Severity::Warning,
                            "pilcrow-action-return",
                            format!("public handler `{name}` accepts Req but does not return ActionResult."),
                            path,
                            Some("crates/routekit/src/templating/codegen/instrument.rs"),
                            Some(
                                "Named page actions should use `pub async fn name(req: Req) -> ActionResult`.",
                            ),
                        ));
                    }
                }
            }
            Item::Const(c) => {
                let name = c.ident.to_string();
                // Validate page option constant values
                if name == "TRAILING_SLASH" {
                    let value_str = quote_const_value(c);
                    if let Some(v) = value_str {
                        if !matches!(v.as_str(), "always" | "never" | "ignore") {
                            findings.push(finding(
                                Severity::Error,
                                "pilcrow-page-option-trailing-slash",
                                format!("TRAILING_SLASH value `{v}` is invalid. Expected: always | never | ignore."),
                                path,
                                Some("crates/routekit/src/templating/page_options.rs"),
                                Some("Use `pub const TRAILING_SLASH: &str = \"always\";` or \"never\" or \"ignore\"."),
                            ));
                        }
                    }
                } else if name == "LAYOUT" {
                    let value_str = quote_const_value(c);
                    if let Some(v) = value_str {
                        if v != "none" {
                            findings.push(finding(
                                Severity::Error,
                                "pilcrow-page-option-layout",
                                format!("LAYOUT value `{v}` is invalid. Only \"none\" is supported."),
                                path,
                                Some("crates/routekit/src/templating/page_options.rs"),
                                Some("Use `pub const LAYOUT: &str = \"none\";` to opt out of layout wrapping."),
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn validate_html(code: &str, path: Option<&str>, findings: &mut Vec<Finding>) {
    for (idx, line) in code.lines().enumerate() {
        let lnum = idx + 1;

        for directive in ["<Island", "client:load", "client:idle", "client:visible", "s-island"] {
            if line.contains(directive) {
                findings.push(finding_with_line(
                    Severity::Error,
                    "pilcrow-planned-islands",
                    format!("`{directive}` depends on planned Islands support and is not valid in stable SSR pages."),
                    path,
                    Some(lnum),
                    Some("registry.toml: feature islands (planned)"),
                    Some("Use server-rendered components and Silcrow enhanced forms/navigation for now."),
                ));
            }
        }

        for directive in ["generateStaticParams", "prerender", "revalidate"] {
            if line.contains(directive) {
                findings.push(finding_with_line(
                    Severity::Error,
                    "pilcrow-planned-static-output",
                    format!("`{directive}` depends on planned SSG or incremental SSR support."),
                    path,
                    Some(lnum),
                    Some("registry.toml: feature ssg, feature incremental-ssr"),
                    Some("Keep the route as an SSR page until static generation is implemented."),
                ));
            }
        }

        if line.contains("useState(") || line.contains("onclick=") || line.contains("x-data") {
            findings.push(finding_with_line(
                Severity::Warning,
                "pilcrow-hydration-static-mismatch",
                "This template appears to depend on client-only state or event handlers without an implemented dynamic feature path.".to_string(),
                path,
                Some(lnum),
                Some("CLAUDE.md: silcrow.js section"),
                Some("Prefer server actions, s-boost navigation, or server-rendered fragments."),
            ));
        }

        // Check for Silcrow directives in .rs files (should not be HTML path for these)
        // Validate action paths: ?/name should reference real action names
        if line.contains("action=\"?/") || line.contains("s-post=\"?/") {
            // Just an info note — we cannot verify action names without the RS file here
            // Only emit if the action appears to use whitespace (malformed)
            if let Some(action) = extract_action_name(line) {
                if action.contains(' ') || action.is_empty() {
                    findings.push(finding_with_line(
                        Severity::Error,
                        "pilcrow-invalid-action-name",
                        format!("Action name `?/{action}` is invalid — names must be valid identifiers."),
                        path,
                        Some(lnum),
                        Some("CLAUDE.md: Actions Pattern section"),
                        Some("Use `?/submit` or `?/create` — a valid Rust identifier after the slash."),
                    ));
                }
            }
        }
    }
}

fn extract_action_name(line: &str) -> Option<String> {
    // Match ?/name in action= or s-post=
    let pos = line.find("?/")?;
    let rest = &line[pos + 2..];
    let end = rest.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

fn quote_const_value(c: &syn::ItemConst) -> Option<String> {
    if let syn::Expr::Lit(expr_lit) = c.expr.as_ref() {
        if let syn::Lit::Str(lit_str) = &expr_lit.lit {
            return Some(lit_str.value());
        }
    }
    None
}

fn returns_named_type(output: &ReturnType, expected: &str) -> bool {
    let ReturnType::Type(_, ty) = output else {
        return false;
    };
    type_contains_name(ty, expected)
}

fn wants_type(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::token::Comma>,
    expected: &str,
) -> bool {
    inputs.iter().any(|arg| {
        let FnArg::Typed(arg) = arg else {
            return false;
        };
        type_contains_name(&arg.ty, expected)
    })
}

fn type_contains_name(ty: &Type, expected: &str) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .iter()
            .any(|segment| segment.ident == expected),
        Type::Reference(reference) => type_contains_name(&reference.elem, expected),
        _ => false,
    }
}

fn looks_like_rust(code: &str) -> bool {
    code.contains("fn ") || code.contains("pub struct") || code.contains("impl ")
}

fn finding(
    severity: Severity,
    rule_id: &str,
    message: String,
    path: Option<&str>,
    source_ref: Option<&str>,
    suggested_fix: Option<&str>,
) -> Finding {
    Finding {
        severity,
        rule_id: rule_id.to_string(),
        message,
        path: path.map(str::to_string),
        line: None,
        source_ref: source_ref.map(str::to_string),
        suggested_fix: suggested_fix.map(str::to_string),
    }
}

fn finding_with_line(
    severity: Severity,
    rule_id: &str,
    message: String,
    path: Option<&str>,
    line: Option<usize>,
    source_ref: Option<&str>,
    suggested_fix: Option<&str>,
) -> Finding {
    Finding {
        severity,
        rule_id: rule_id.to_string(),
        message,
        path: path.map(str::to_string),
        line,
        source_ref: source_ref.map(str::to_string),
        suggested_fix: suggested_fix.map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_planned_island_directive() {
        let report =
            validate_implementation("<Island client:load />", Some("src/pages/index.html"), None);
        assert!(!report.valid);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.rule_id == "pilcrow-planned-islands"));
    }

    #[test]
    fn accepts_current_load_shape() {
        let report = validate_implementation(
            "pub struct Props {}\npub async fn load(req: Req) -> AppResult<Props> { Ok(Props {}) }",
            Some("src/pages/index.rs"),
            None,
        );
        assert!(report.valid, "{:?}", report.findings);
    }

    #[test]
    fn rejects_non_async_load() {
        let report = validate_implementation(
            "pub struct Props {}\npub fn load(req: Req) -> AppResult<Props> { Ok(Props {}) }",
            Some("src/pages/index.rs"),
            None,
        );
        assert!(!report.valid);
        assert!(report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-load-async"));
    }

    #[test]
    fn rejects_invalid_trailing_slash_value() {
        let report = validate_implementation(
            "pub const TRAILING_SLASH: &str = \"redirect\";",
            Some("src/pages/index.rs"),
            None,
        );
        assert!(!report.valid);
        assert!(report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-page-option-trailing-slash"));
    }

    #[test]
    fn rejects_layout_with_action() {
        let report = validate_implementation(
            "pub async fn create(req: Req) -> ActionResult { redirect(\"/\") }",
            Some("src/pages/_layout.rs"),
            None,
        );
        // This should warn about actions in layout
        assert!(report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-layout-no-actions" || f.rule_id == "pilcrow-action-return"));
    }

    #[test]
    fn finding_has_source_ref() {
        let report = validate_implementation(
            "pub fn load(req: Req) -> AppResult<Props> { Ok(Props {}) }",
            Some("src/pages/index.rs"),
            None,
        );
        let async_finding = report.findings.iter().find(|f| f.rule_id == "pilcrow-load-async");
        assert!(async_finding.is_some());
        assert!(async_finding.unwrap().source_ref.is_some());
    }

    #[test]
    fn finding_has_line_number_for_html() {
        let html = "line1\n<Island client:load />\nline3";
        let report = validate_implementation(html, Some("src/pages/index.html"), None);
        let f = report
            .findings
            .iter()
            .find(|f| f.rule_id == "pilcrow-planned-islands")
            .unwrap();
        assert_eq!(f.line, Some(2));
    }
}
