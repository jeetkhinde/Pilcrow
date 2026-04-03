# Phases 8, 9, 10 — Proposed Roadmap

## Context

Phases 1–7 hardened the framework internals (build correctness, API tightening, routekit,
pilcrow-web facade, naming, tests, api/ routing). The folder-structure refactor followed.
The framework compiles and passes clippy clean. The next three phases shift focus from
framework internals to **proving it works end-to-end**, **handling failure gracefully**, and
**improving the developer loop**.

---

## Phase 8 — Sandbox

**Goal:** Recreate `sandbox/` as the canonical reference app. Uses Pilcrow as an external
consumer (not part of the framework). Proves the full flow works together and gives newcomers
a learning path.

**What it demonstrates:**
- `build.rs` calling `routekit::compile_to_out_dir` + `watched_source_directories`
- `.html` templates with `pub struct Props` frontmatter
- File-based page routes (`pages/`) + API routes (`api/`)
- BFF pattern: page handler fetches from backend via `api-client-rest`, maps DTO → Props
- SSE stream (live counter or notification feed)
- `ResponseExt` chaining: toasts, retarget, push_history
- `pilcrow-cli check-arch` passes on the sandbox workspace

**Scope:**
| File | Purpose |
|---|---|
| `sandbox/Cargo.toml` | Workspace root — NOT part of framework workspace |
| `sandbox/apps/web/` | BFF + SSR (pages, api/, build.rs) |
| `sandbox/apps/backend/` | Minimal service (in-memory todos) |
| `sandbox/crates/contracts/` | TodoDto, CreateTodoRequest |

**Not included:** database, auth, production config. Those come after.

---

## Phase 9 — Error Boundaries

**Goal:** Turn the already-detected `_error.html` and `not-found.html` route markers into
first-class rendered responses. Currently routekit detects these files but the pipeline
does nothing with them beyond metadata.

**Problem:** If a handler panics or returns an error, there is no framework-level fallback.
Apps must wire their own Axum fallback. API routes return raw `StatusCode` errors with no
envelope.

**What changes:**

### routekit
- `_error.html` pages get render functions generated (like page routes, same Props injection)
- `not-found.html` pages same treatment
- Expose `generated_error_routes()` and `generated_not_found_route()` from the manifest

### pilcrow (runtime)
- `ErrorBoundary` middleware: catches handler errors, looks up nearest `_error.html` render
  function, returns rendered HTML or JSON `ApiErrorBody` based on `RequestMode`
- `error_boundary(router)` convenience wrapper (thin Axum layer)
- API routes: `AppError` → structured `ApiErrorBody` JSON with correct status codes
  (reuse the `app_error_to_response` pattern already in `apps/backend/src/middleware.rs`)

### pilcrow-web facade
- Re-export `error_boundary`, `ErrorBoundary`

**Critical files:**
- `crates/routekit/src/templating/pipeline.rs` — add error/not-found page codegen
- `crates/routekit/src/templating/codegen.rs` — `render_generated_error_routes_module`
- `crates/runtime/src/` — new `error_boundary.rs` module
- `crates/web/src/lib.rs` — re-export new symbols

---

## Phase 10 — `pilcrow dev` (Dev Server)

**Goal:** Add a `pilcrow dev` subcommand to the CLI that starts the app with template
hot-reload. The single biggest developer experience gap: today you must `cargo run` manually
and restart on every template change.

**What it does:**
1. Runs `cargo build` once upfront
2. Spawns the app binary as a child process
3. Watches `src/pages/`, `src/components/`, `src/layouts/`, `src/api/` for changes
4. On `.html` change: re-runs `routekit::compile_to_out_dir` and signals the app to
   reload templates (or restarts the process if needed)
5. On `.rs` change: runs `cargo build` and restarts

**Scope (v1 — process restart only):**
- `pilcrow dev [--port 3000] [--manifest-dir .]`
- Uses `std::process::Command` to spawn `cargo run`
- Uses `notify` crate for filesystem watching
- Prints clean output: `[pilcrow] template changed → reloading`, `[pilcrow] ready on :3000`

**Out of scope for v1:** in-process reload, browser live-refresh, HMR.

**Critical files:**
- `tools/cli/src/main.rs` — add `dev` subcommand
- `tools/cli/Cargo.toml` — add `notify` dependency
- New file: `tools/cli/src/dev.rs`

---

## Recommended Order

8 → 9 → 10. The sandbox (8) proves all prior phases work together and reveals gaps.
Error boundaries (9) make the framework suitable for real apps. Dev server (10) is pure DX
and can ship independently once 8 and 9 validate the architecture.
