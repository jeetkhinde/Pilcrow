# Pilcrow AI-Native MCP Server v1

## Summary

- Add a standalone Rust MCP binary at `tools/mcp/pilcrow-mcp`, using the official Rust MCP SDK `rmcp` over stdio.
- Add a root-level `registry.toml` as Pilcrow's dynamic feature source of truth; the server reads it at runtime and status-gates planned features.
- Update `.mcp.json` to register the new Rust server while keeping the current JS codegen inspector temporarily available.
- Report both MCP server version and Pilcrow framework crate versions, with `pilcrow-web` as the primary framework version.

References used: official MCP SDK list and Rust SDK docs: https://modelcontextprotocol.io/docs/sdk, https://github.com/modelcontextprotocol/rust-sdk

## Architecture Decision

- Build the source-of-truth MCP server in Rust.
- Keep the MCP protocol layer thin so `rmcp` SDK churn is isolated from Pilcrow-specific logic.
- Implement registry loading, project scanning, validation, scaffolding, and codegen inspection as regular Rust modules.
- Keep any existing JS MCP/codegen inspector only as a temporary compatibility or reference surface.
- Add a TypeScript wrapper later only if host compatibility or SDK maturity becomes a practical blocker.

Rationale: TypeScript currently has the more mature Tier 1 MCP SDK, but Pilcrow's MCP server needs to understand Rust crates, `Cargo.toml`, `Pilcrow.toml`, generated route artifacts, templates, and framework conventions. Keeping the domain logic in Rust avoids duplicating Pilcrow semantics in JavaScript.

## Key Changes

- Registry schema:
  - `registry_schema_version`, `framework_version`, and `[[features]]`.
  - Each feature has `id`, `name`, `domain = "pilcrow" | "silcrow"`, `status = "stable" | "experimental" | "planned" | "deprecated"`, `summary`, `spec`, `validation_rules`, and `scaffold_templates`.
  - Seed implemented features from current repo behavior: SSR pages, file routing, layouts, route groups, fragments, API routes, actions, Silcrow enhanced navigation/forms, deferred streams, middleware, env config, loading skeletons, page options.
  - Include Islands, SSG, and Incremental SSR as `planned`, so agents can discuss them but validation/scaffolding rejects unsupported syntax.

- MCP capabilities:
  - `list_features({ status?, domain? })` returns JSON from `registry.toml`.
  - `get_feature_spec({ id })` returns one full feature spec and its usage constraints.
  - `scan_project_context({ project_root?, manifest_path? })` returns routes, layouts, UI components, fragments, APIs, params, middleware, `Pilcrow.toml`, crate versions, and generated OUT_DIR status.
  - Resource `pilcrow://current-project` returns the same project map as structured JSON.
  - `validate_implementation({ code, path?, kind?, project_root? })` returns a code-review report with severities, rule ids, messages, and suggested fixes.
  - `suggest_optimizations({ project_root?, focus? })` analyzes the project map and recommends Pilcrow/Silcrow architecture improvements.
  - `orchestrate_feature({ kind, name, route_path?, target_dir?, options?, dry_run? })` writes routes, components, fragments, or Silcrow integrations with path containment checks, collision detection, FP-first templates, and a dry-run preview.
  - Add Rust equivalents or aliases for current inspector workflows: `codegen_build`, `codegen_list`, and `codegen_read`.

- Validation behavior:
  - Parse Rust snippets with `syn`; inspect templates with lightweight HTML/directive scanning.
  - Enforce current code-behind conventions: async `load(req: Req) -> AppResult<Props>`, named actions returning `ActionResult`, no manual route registration, no unsupported planned feature syntax.
  - Flag Pilcrow/Silcrow boundary mistakes, such as Silcrow client directives in Rust code or planned Island directives in stable SSR pages.
  - Flag "hydration/static mismatch" when a static page pattern attempts stateful/client-only behavior without an implemented dynamic feature path.

- Scaffolding behavior:
  - Route scaffold creates paired `.html` and optional `.rs` code-behind using existing Pilcrow conventions.
  - Component scaffold creates `src/ui/*.html` templates with import-friendly names.
  - Fragment scaffold respects `Pilcrow.toml` `[[fragments]]`; if no fragment dir exists, use `src/widgets` and update config during write mode.
  - Silcrow integration scaffold emits server-backed enhanced forms/navigation patterns, not standalone client-state islands.

## Test Plan

- `cargo test --manifest-path tools/mcp/pilcrow-mcp/Cargo.toml`
- Unit tests for registry parsing, status filtering, feature lookup, version reporting, path safety, validation rules, and scaffold collision handling.
- Temp-project integration tests for project scanning, `pilcrow://current-project`, and scaffold write/dry-run output.
- Compatibility checks: `cargo test -p pilcrow-routekit` and `cargo build --manifest-path sandbox/apps/web/Cargo.toml`.
- Smoke-test the MCP server through stdio by listing tools/resources and calling one tool from each capability group.

## Assumptions

- The new server lives under `tools/mcp/pilcrow-mcp` and is not added to the root workspace.
- `.mcp.json` runs it via `cargo run --quiet --manifest-path tools/mcp/pilcrow-mcp/Cargo.toml --`.
- Scaffolding tools may mutate files, but must default to safe writes: no overwrite unless explicitly requested, all paths confined to the detected project root.
- Implementation finishes with a developer-level test run and a descriptive commit, e.g. `feat(mcp): add Pilcrow AI-native server`.
