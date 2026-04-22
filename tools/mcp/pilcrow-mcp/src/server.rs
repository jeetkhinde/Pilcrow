use crate::{
    codegen,
    registry::{FeatureDomain, FeatureStatus, Registry},
    scaffold::{orchestrate_feature, ScaffoldRequest},
    validation::validate_implementation,
    workspace::scan_project,
};
use anyhow::{Context, Result};
use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars::JsonSchema,
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

const CURRENT_PROJECT_URI: &str = "pilcrow://current-project";

#[derive(Clone)]
pub struct PilcrowServer {
    project_root: PathBuf,
    registry: Registry,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ListFeaturesArgs {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FeatureSpecArgs {
    pub id: String,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ProjectArgs {
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub manifest_path: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ValidateArgs {
    pub code: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub project_root: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SuggestArgs {
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub manifest_path: Option<String>,
    #[serde(default)]
    pub focus: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct OrchestrateArgs {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub route_path: Option<String>,
    #[serde(default)]
    pub target_dir: Option<String>,
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub manifest_path: Option<String>,
    #[serde(default)]
    pub options: Option<Value>,
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(default)]
    pub overwrite: Option<bool>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct CodegenBuildArgs {
    #[serde(default)]
    pub manifest: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct CodegenListArgs {
    #[serde(default)]
    pub manifest: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CodegenReadArgs {
    pub file: String,
    #[serde(default)]
    pub manifest: Option<String>,
    #[serde(default)]
    pub head: Option<usize>,
    #[serde(default)]
    pub tail: Option<usize>,
}

#[derive(Debug, Serialize)]
struct Optimizations {
    focus: Option<String>,
    recommendations: Vec<Optimization>,
}

#[derive(Debug, Serialize)]
struct Optimization {
    rule_id: &'static str,
    severity: &'static str,
    message: String,
    suggested_fix: String,
}

#[tool_router]
impl PilcrowServer {
    pub fn new() -> Result<Self> {
        let cwd = std::env::current_dir().context("failed to read current directory")?;
        let project_root = crate::registry::find_project_root(&cwd).unwrap_or(cwd);
        let registry = Registry::load_from_project(&project_root)?;
        Ok(Self {
            project_root,
            registry,
            tool_router: Self::tool_router(),
        })
    }

    #[tool(
        description = "List Pilcrow/Silcrow feature registry entries, optionally filtered by status or domain."
    )]
    async fn list_features(
        &self,
        Parameters(args): Parameters<ListFeaturesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let status = match args.status.as_deref().map(parse_status).transpose() {
            Ok(status) => status,
            Err(error) => return Ok(tool_error(error)),
        };
        let domain = match args.domain.as_deref().map(parse_domain).transpose() {
            Ok(domain) => domain,
            Err(error) => return Ok(tool_error(error)),
        };
        Ok(structured(self.registry.filtered(status, domain)))
    }

    #[tool(
        description = "Return one full Pilcrow/Silcrow feature spec and its validation/scaffolding constraints."
    )]
    async fn get_feature_spec(
        &self,
        Parameters(args): Parameters<FeatureSpecArgs>,
    ) -> Result<CallToolResult, McpError> {
        match self.registry.feature(&args.id) {
            Some(feature) => Ok(structured(feature)),
            None => Ok(tool_error(format!("unknown feature id: {}", args.id))),
        }
    }

    #[tool(
        description = "Scan a Pilcrow project for routes, layouts, components, fragments, APIs, params, middleware, config, crate versions, and generated OUT_DIR status."
    )]
    async fn scan_project_context(
        &self,
        Parameters(args): Parameters<ProjectArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match scan_project(
                &self.project_root,
                args.project_root.as_deref(),
                args.manifest_path.as_deref(),
            ) {
                Ok(context) => structured(context),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(
        description = "Validate Rust or HTML snippets against current Pilcrow/Silcrow conventions and planned-feature gates."
    )]
    async fn validate_implementation(
        &self,
        Parameters(args): Parameters<ValidateArgs>,
    ) -> Result<CallToolResult, McpError> {
        let report =
            validate_implementation(&args.code, args.path.as_deref(), args.kind.as_deref());
        Ok(structured(report))
    }

    #[tool(
        description = "Analyze the current project map and recommend Pilcrow/Silcrow architecture improvements."
    )]
    async fn suggest_optimizations(
        &self,
        Parameters(args): Parameters<SuggestArgs>,
    ) -> Result<CallToolResult, McpError> {
        let context = match scan_project(
            &self.project_root,
            args.project_root.as_deref(),
            args.manifest_path.as_deref(),
        ) {
            Ok(context) => context,
            Err(error) => return Ok(tool_error(error.to_string())),
        };
        Ok(structured(suggest_from_context(&context, args.focus)))
    }

    #[tool(
        description = "Scaffold a route, component, fragment, or server-backed Silcrow pattern with path containment, collision detection, and dry-run support."
    )]
    async fn orchestrate_feature(
        &self,
        Parameters(args): Parameters<OrchestrateArgs>,
    ) -> Result<CallToolResult, McpError> {
        let request = ScaffoldRequest {
            kind: &args.kind,
            name: &args.name,
            route_path: args.route_path.as_deref(),
            target_dir: args.target_dir.as_deref(),
            options: args.options.as_ref(),
            dry_run: args.dry_run.unwrap_or(true),
            overwrite: args.overwrite.unwrap_or(false),
        };
        Ok(
            match orchestrate_feature(
                &self.project_root,
                args.project_root.as_deref(),
                args.manifest_path.as_deref(),
                request,
            ) {
                Ok(result) => structured(result),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(
        description = "Build a Pilcrow web app and report generated routekit OUT_DIR artifacts."
    )]
    async fn codegen_build(
        &self,
        Parameters(args): Parameters<CodegenBuildArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match codegen::codegen_build(&self.project_root, args.manifest.as_deref()) {
                Ok(result) => {
                    if result.success {
                        structured(result)
                    } else {
                        CallToolResult::structured_error(json!(result))
                    }
                }
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(description = "List files generated by the Pilcrow routekit build pipeline in OUT_DIR.")]
    async fn codegen_list(
        &self,
        Parameters(args): Parameters<CodegenListArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match codegen::codegen_list(&self.project_root, args.manifest.as_deref()) {
                Ok(result) => structured(result),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(
        description = "Read a generated OUT_DIR file, optionally limited with head or tail line counts."
    )]
    async fn codegen_read(
        &self,
        Parameters(args): Parameters<CodegenReadArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match codegen::codegen_read(
                &self.project_root,
                args.manifest.as_deref(),
                &args.file,
                args.head,
                args.tail,
            ) {
                Ok(result) => structured(result),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }
}

#[tool_handler]
impl ServerHandler for PilcrowServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.protocol_version = ProtocolVersion::LATEST;
        info.capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()
            .build();
        info.server_info = Implementation::from_build_env();
        info.instructions = Some(
            "Pilcrow AI-native MCP server. Use registry tools for feature status, scan_project_context before editing, validate_implementation before scaffolding planned syntax, and orchestrate_feature for safe writes.".to_string(),
        );
        info
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![RawResource::new(CURRENT_PROJECT_URI, "current-project")
                .with_title("Pilcrow current project")
                .with_description(
                    "Structured scan of the current Pilcrow app and generated artifact status.",
                )
                .with_mime_type("application/json")
                .no_annotation()],
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        if request.uri != CURRENT_PROJECT_URI {
            return Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({ "uri": request.uri })),
            ));
        }
        let context = scan_project(&self.project_root, None, None)
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        let text = serde_json::to_string_pretty(&context)
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            text,
            CURRENT_PROJECT_URI,
        )
        .with_mime_type("application/json")]))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![],
            next_cursor: None,
            meta: None,
        })
    }
}

fn structured(value: impl Serialize) -> CallToolResult {
    match serde_json::to_value(value) {
        Ok(value) => CallToolResult::structured(value),
        Err(error) => tool_error(error.to_string()),
    }
}

fn tool_error(message: impl Into<String>) -> CallToolResult {
    CallToolResult::structured_error(json!({ "error": message.into() }))
}

fn parse_status(value: &str) -> std::result::Result<FeatureStatus, String> {
    match value {
        "stable" => Ok(FeatureStatus::Stable),
        "experimental" => Ok(FeatureStatus::Experimental),
        "planned" => Ok(FeatureStatus::Planned),
        "deprecated" => Ok(FeatureStatus::Deprecated),
        other => Err(format!("unknown feature status: {other}")),
    }
}

fn parse_domain(value: &str) -> std::result::Result<FeatureDomain, String> {
    match value {
        "pilcrow" => Ok(FeatureDomain::Pilcrow),
        "silcrow" => Ok(FeatureDomain::Silcrow),
        other => Err(format!("unknown feature domain: {other}")),
    }
}

fn suggest_from_context(
    context: &crate::workspace::ProjectContext,
    focus: Option<String>,
) -> Optimizations {
    let mut recommendations = Vec::new();
    if context.loading_skeletons.is_empty() {
        recommendations.push(Optimization {
            rule_id: "pilcrow-loading-skeletons",
            severity: "info",
            message: "No _loading.html templates were found.".to_string(),
            suggested_fix: "Add scoped _loading.html templates near routes that fetch remote data or use deferred streams.".to_string(),
        });
    }
    if context.middleware.is_none() {
        recommendations.push(Optimization {
            rule_id: "pilcrow-middleware",
            severity: "info",
            message: "No src/middleware.rs was found.".to_string(),
            suggested_fix: "Add middleware only when cross-cutting auth, headers, tracing, or request locals are needed.".to_string(),
        });
    }
    if context.fragments.is_empty() {
        recommendations.push(Optimization {
            rule_id: "pilcrow-fragments",
            severity: "warning",
            message: "No fragment groups are configured in Pilcrow.toml.".to_string(),
            suggested_fix: "Configure [[fragments]] when repeated UI needs addressable server-rendered partials.".to_string(),
        });
    }
    if context.generated_out_dir.path.is_none() {
        recommendations.push(Optimization {
            rule_id: "pilcrow-codegen-status",
            severity: "warning",
            message: context.generated_out_dir.message.clone(),
            suggested_fix: "Run codegen_build before inspecting generated routes or typed helpers."
                .to_string(),
        });
    }
    if matches!(focus.as_deref(), Some("silcrow")) {
        recommendations.push(Optimization {
            rule_id: "silcrow-server-backed-interactions",
            severity: "info",
            message: "Silcrow interactions should preserve plain HTML behavior and use server actions for state.".to_string(),
            suggested_fix: "Prefer forms posting to ?/action and anchors with s-boost/s-target over client-only state.".to_string(),
        });
    }
    Optimizations {
        focus,
        recommendations,
    }
}
