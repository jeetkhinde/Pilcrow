use serde::Serialize;
use syn::{FnArg, Item, ReturnType, Type, Visibility};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    for directive in [
        "s-boost",
        "s-target",
        "s-swap",
        "s-trigger",
        "client:",
        "<Island",
    ] {
        if code.contains(directive) {
            findings.push(finding(
                Severity::Error,
                "pilcrow-boundary-silcrow-in-rust",
                format!("Silcrow/client directive `{directive}` belongs in HTML templates, not Rust code."),
                path,
                Some("Move the directive to the paired .html template."),
            ));
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
            ));
            return;
        }
    };

    for item in parsed.items {
        let Item::Fn(function) = item else {
            continue;
        };
        let name = function.sig.ident.to_string();
        if name == "load" {
            if function.sig.asyncness.is_none() {
                findings.push(finding(
                    Severity::Error,
                    "pilcrow-load-async",
                    "load must be declared async.".to_string(),
                    path,
                    Some("Use `pub async fn load(req: Req) -> AppResult<Props>`."),
                ));
            }
            if !returns_named_type(&function.sig.output, "AppResult") {
                findings.push(finding(
                    Severity::Error,
                    "pilcrow-load-return",
                    "load should return AppResult<Props>.".to_string(),
                    path,
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
                    Some("Use `pub async fn load(req: Req) -> AppResult<Props>`."),
                ));
            }
        } else if matches!(function.vis, Visibility::Public(_))
            && wants_type(&function.sig.inputs, "Req")
            && !returns_named_type(&function.sig.output, "ActionResult")
        {
            findings.push(finding(
                Severity::Warning,
                "pilcrow-action-return",
                format!("public handler `{name}` accepts Req but does not return ActionResult."),
                path,
                Some(
                    "Named page actions should use `pub async fn name(req: Req) -> ActionResult`.",
                ),
            ));
        }
    }
}

fn validate_html(code: &str, path: Option<&str>, findings: &mut Vec<Finding>) {
    for directive in [
        "<Island",
        "client:load",
        "client:idle",
        "client:visible",
        "s-island",
    ] {
        if code.contains(directive) {
            findings.push(finding(
                Severity::Error,
                "pilcrow-planned-islands",
                format!("`{directive}` depends on planned Islands support and is not valid in stable SSR pages."),
                path,
                Some("Use server-rendered components and Silcrow enhanced forms/navigation for now."),
            ));
        }
    }
    for directive in ["generateStaticParams", "prerender", "revalidate"] {
        if code.contains(directive) {
            findings.push(finding(
                Severity::Error,
                "pilcrow-planned-static-output",
                format!("`{directive}` depends on planned SSG or incremental SSR support."),
                path,
                Some("Keep the route as an SSR page until static generation is implemented."),
            ));
        }
    }
    if code.contains("useState(") || code.contains("onclick=") || code.contains("x-data") {
        findings.push(finding(
            Severity::Warning,
            "pilcrow-hydration-static-mismatch",
            "This template appears to depend on client-only state or event handlers without an implemented dynamic feature path.".to_string(),
            path,
            Some("Prefer server actions, s-boost navigation, or server-rendered fragments."),
        ));
    }
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
    suggested_fix: Option<&str>,
) -> Finding {
    Finding {
        severity,
        rule_id: rule_id.to_string(),
        message,
        path: path.map(str::to_string),
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
}
