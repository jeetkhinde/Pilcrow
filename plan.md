# Pilcrow Expert MCP Roadmap

## Goal

Make `tools/mcp/pilcrow-mcp` the authoritative Pilcrow-side expert MCP server.
It should answer Pilcrow framework questions with repo-grounded evidence, inspect
projects deeply, validate implementations semantically, diagnose build/codegen
issues, and scaffold production-grade Pilcrow patterns.

`silcrow-mcp` already owns `silcrow.js` expertise. Pilcrow MCP must not duplicate
that surface. It should understand Pilcrow's server-side integration points with
Silcrow, then defer detailed client-runtime behavior to `silcrow-mcp`.

## Ownership Boundary

- Pilcrow MCP owns:
  - SSR pages and code-behind conventions.
  - routekit routing, route discovery, route groups, typed params, generated code.
  - Layouts, loading/error/not-found pages, fragments, UI templates, slots.
  - API routes, actions, middleware, env config, page options.
  - Deferred streams and Pilcrow runtime/web facade APIs.
  - Pilcrow project scanning, validation, diagnostics, scaffolding, and generated artifact inspection.

- `silcrow-mcp` owns:
  - `silcrow.js` directives and exact client-side semantics.
  - DOM patching, enhanced navigation/form runtime behavior, cache behavior, live/SSE/WS client semantics.

- Pilcrow MCP bridge behavior:
  - Validate that Silcrow-facing Pilcrow code has the right server-side shape.
  - Clearly report when a question needs `silcrow-mcp` for client-runtime details.
  - Avoid claiming detailed Silcrow behavior from memory or duplicated docs.

## Milestone 1: Knowledge Base And Resources ✅

- Index authoritative local sources:
  - `CLAUDE.md` ✅
  - `crates/routekit/README.md` ✅
  - crate-level docs and public API comments ✅
  - routekit/runtime/web tests ✅
  - sandbox examples ✅
  - generated-code patterns from `OUT_DIR` ✅

- Expand `registry.toml` from summary feature entries into detailed specs: ✅
  - canonical usage ✅
  - constraints ✅
  - invalid examples ✅
  - source/test references ✅
  - scaffold support status ✅
  - Silcrow boundary notes where relevant ✅

- Add resources: ✅
  - `pilcrow://docs` ✅
  - `pilcrow://api/runtime` ✅
  - `pilcrow://api/web` ✅
  - `pilcrow://routekit/features` ✅
  - `pilcrow://examples` ✅
  - `pilcrow://tests/feature-matrix` ✅
  - Keep `pilcrow://current-project`. ✅

## Milestone 2: Deep Project Model ✅

- Upgrade `scan_project_context` from file lists to a semantic project graph: ✅
  - route graph with URL patterns, route groups, dynamic params, catch-all routes ✅
  - layout chain per route ✅
  - loading/error/not-found coverage ✅
  - template imports, component usage, slots, fragments ✅ (via inspect_template)
  - code-behind metadata: `Props`, `load`, named actions, deferred fields, imports, page options ✅
  - API route files and exported router symbols ✅
  - middleware/env config detection ✅
  - generated `OUT_DIR` status cross-checked against source routes ✅

- Add targeted scan tools: ✅
  - `inspect_route({ route })` ✅
  - `inspect_template({ path })` ✅
  - `inspect_code_behind({ path })` ✅
  - `inspect_generated_route({ route })` ✅

## Milestone 3: Semantic Validation ✅

- Replace lightweight validation with compiler-aware checks: ✅
  - Use `syn` for Rust AST validation. ✅
  - Reuse routekit parser/compiler modules where practical. — skipped (would require adding pilcrow-routekit as dependency)
  - Validate HTML/template syntax with routekit's compiler logic where practical. — skipped (same reason)

- Validate: ✅
  - `load(req: Req) -> AppResult<Props>` shape ✅
  - named actions returning `ActionResult` ✅
  - `Props` fields against template usage — partial (field names extracted; cross-referencing template vars not implemented)
  - deferred fields and streaming support ✅
  - page option constants ✅ (TRAILING_SLASH values, LAYOUT values)
  - middleware signatures — partial (detected via diagnostics, not AST-validated)
  - API route `router()` exports ✅
  - fragment config/path consistency ✅
  - component imports and slot usage ✅ (via inspect_template)
  - planned-feature gates for Islands, SSG, Incremental SSR ✅
  - Pilcrow/Silcrow boundary mistakes ✅

- Findings should include: ✅
  - severity ✅
  - stable rule ID ✅
  - file/path ✅
  - location when available ✅ (line numbers)
  - source/test reference when available ✅
  - concrete suggested fix ✅

## Milestone 4: Expert Q&A Tools ✅

- Add tools intended for agent answers: ✅
  - `answer_pilcrow_question({ question, project_root? })` ✅
  - `explain_feature({ id, depth?, include_examples? })` ✅
  - `find_examples({ feature, pattern? })` ✅
  - `compare_patterns({ goal, options? })` ✅
  - `why_build_failed({ manifest?, error_log? })` ✅

- Answers should be grounded in indexed docs/tests/examples. ✅
- Answers should distinguish implemented, experimental, planned, and unsupported behavior. ✅
- Questions about exact `silcrow.js` runtime behavior should return a delegation note to use `silcrow-mcp`. ✅

## Milestone 5: Production-Grade Scaffolding ✅

- Expand `orchestrate_feature` scaffolds: ✅
  - static SSR page ✅ (`static-page`)
  - loaded SSR page ✅ (`loaded-page`)
  - action-backed page ✅ (`action-page`)
  - deferred page ✅ (`deferred-page`)
  - nested layout ✅ (`nested-layout`)
  - loading/error/not-found page ✅ (`loading-page`, `error-page`, `not-found-page`)
  - API route ✅ (`api-route`)
  - middleware ✅ (`middleware`)
  - env config ✅ (`env-config`)
  - fragment ✅ (`fragment`)
  - UI component ✅ (`component`)
  - typed param ✅ (`typed-param`)

- Scaffolds must: ✅
  - default to dry-run ✅
  - prevent path escape ✅
  - detect collisions ✅
  - preserve existing files unless overwrite is explicit ✅
  - match current Pilcrow conventions ✅
  - include validation after generation ✅ (validation_notes returned with every scaffold)

- For Silcrow-enhanced flows: ✅
  - Generate Pilcrow server-side actions/templates only. ✅
  - Mark exact client runtime behavior as owned by `silcrow-mcp`. ✅

## Milestone 6: Diagnostics And Repair ✅

- Add diagnostic tools: ✅
  - `diagnose_project({ project_root?, manifest_path? })` ✅
  - `diagnose_route({ route })` ✅
  - `diagnose_codegen({ manifest? })` ✅
  - `propose_fix({ finding_id })` ✅
  - `apply_safe_fix({ finding_id, dry_run? })` ✅

- Diagnostics should connect: ✅
  - source files ✅
  - routekit discovery/compiler behavior ✅ (via rule source_refs)
  - generated `OUT_DIR` files ✅
  - Cargo build errors ✅ (via why_build_failed)
  - validation findings ✅

- Repair tools must remain conservative: ✅
  - dry-run by default ✅
  - no unrelated rewrites ✅
  - no destructive edits ✅
  - explicit collision reporting ✅

## Milestone 7: Evaluations ⬜

- Add fixture apps and golden tests for:
  - route graph extraction
  - layout chains
  - invalid `load` functions
  - invalid named actions
  - bad template imports
  - fragment config edge cases
  - deferred fields
  - middleware
  - API routes
  - generated-code inspection
  - Silcrow boundary/delegation behavior

- Add MCP stdio smoke tests for:
  - listing tools/resources
  - reading every resource
  - calling one tool from every capability group
  - dry-run and write-mode scaffolding
  - error responses for unsupported/planned features

- Add expert benchmark questions:
  - "How do I add nested layouts?"
  - "Why is my action not discovered?"
  - "Why does this route not render?"
  - "How do I add a fragment directory?"
  - "What generated file should I inspect for this route?"
  - "Is this Silcrow behavior or Pilcrow behavior?"

## Milestone 8: Integration Polish 🔶

- Add MCP prompts: ✅
  - Pilcrow code review ✅
  - Pilcrow scaffold ✅
  - Pilcrow build diagnosis ✅
  - Pilcrow feature explanation ✅

- Keep MCP protocol code thin: ✅
  - `src/server.rs` handles transport/tool/resource registration. ✅
  - domain logic stays in normal modules. ✅
  - `rmcp` churn should not leak into validators/scanners/scaffolders. ✅

- Retire `tools/mcp/codegen-inspector` only after Rust MCP reaches complete parity. ⬜ (blocked on Milestone 7)

## Definition Of Done

- Pilcrow MCP can answer common and advanced Pilcrow questions from local evidence. ✅
- It can distinguish Pilcrow, Silcrow, implemented, planned, and unsupported concerns. ✅
- It can diagnose route/codegen/build issues without guessing. ✅
- It can validate and scaffold all currently implemented Pilcrow patterns. ✅
- It has fixture-backed tests and MCP smoke tests for every tool/resource. ⬜ (Milestone 7)
- It delegates exact `silcrow.js` runtime questions to `silcrow-mcp`. ✅
