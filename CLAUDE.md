# Pilcrow — Agent Guide

This is the source of truth for any coding agent working on Pilcrow.
Read this before writing any code.

## What Pilcrow Is

An Astro-like web framework for Rust.
Templates are `.html` files with Rust frontmatter. The compiler turns them into
Askama-derived render functions at build time. The runtime is Axum.

## Core Architectural Decisions

### 1. Frontend and backend are independent

- Frontend (`apps/web`) works with or without a backend.
- Backend (`apps/backend`) works with or without a frontend.
- A static site needs no backend. A dashboard needs one. Same framework.

### 2. Browser never talks to backend directly

```
Browser --> apps/web/pages/   --> (optional) backend --> HTML response
Browser --> apps/web/api/     --> (optional) backend --> JSON response
```

- `pages/` directory = file-based routes returning HTML via templates.
- `api/` directory = file-based routes returning JSON via handlers.
- Both live inside the web app. Both proxy to backend when needed.
- Backend is never exposed to the browser.

### 3. Props are not DTOs

| Concept | Where it lives | Purpose | Serialized? |
|---------|---------------|---------|-------------|
| **Props** | Inside the `.html` template file | What the template needs to render | Never |
| **DTO** | `apps/web` and `apps/backend` API modules | What crosses the network between web and backend | Always |

- Templates never import from contracts.
- The web handler maps DTO -> Props. That is its only job.
- If no backend exists, the handler builds Props from local data.

### 4. One template syntax

```
---
pub struct Props {
    pub title: String,
    pub items: Vec<TodoItem>,
}
---
<h1>{{ title }}</h1>
<ul>
  {% for item in items %}
  <li>{{ item.name }}</li>
  {% endfor %}
</ul>
```

- `---` fences contain simple Rust code. `pub struct Props` is mandatory.
- Everything after the closing `---` is HTML (Askama syntax).
- Templates produce HTML. They never produce JSON.
- JSON responses come from handlers in the `api/` directory.

## Workspace Structure

```
Pilcrow/
  crates/
    pilcrow-runtime/      # Low-level response/runtime (Axum layer)
    pilcrow-core/         # Domain primitives (envelope, error types)
    pilcrow-web/          # Curated facade for web app developers
    pilcrow-macros/       # Proc macros (sse)
    routekit/             # File-based route + template compiler
  sandbox/
    apps/
      web/                # BFF + SSR (pages, api, components, layouts)
      backend/            # Domain logic, services, DB, auth, REST APIs
  tools/
    pilcrow-cli/          # Scaffolding + architecture validation
```

## Framework Crate Responsibilities

### routekit (the compiler)
- Discovers `.html` files in `pages/`, `components/`, `layouts/`.
- Splits frontmatter from template.
- Expands `<Component />` tags, resolves slots (default, named, let-bindings).
- Emits `generated_routes.rs` and `generated_templates.rs`.
- Called from `build.rs`. Never at runtime.

### pilcrow (the runtime)
- Content negotiation (`SilcrowRequest`, `RequestMode`).
- Response builders (`json()`, `navigate()`, `status()`).
- `ResponseExt` modifiers (toasts, headers, retarget, SSE, WS headers).
- `json()` + `ResponseExt` chaining replaces the old `respond!()` macro.
- SSE and WebSocket support.
- Silcrow.js asset serving.

### pilcrow-web (the facade)
- Curated re-exports from `pilcrow` and `pilcrow-core`.
- This is what `apps/web` depends on. No wildcard re-exports.
- Only expose what a web app developer actually needs.

### pilcrow-core (domain primitives)
- `ApiEnvelope`, `AppError`, `AppResult`.
- This is what `apps/backend` depends on.
- No web/UI/template imports allowed.

### App transport types
- DTOs are app-layer wire types.
- Keep web and backend transport contracts at app boundaries.
- Handlers map transport DTOs to template Props explicitly.

## Dependency Boundaries (enforced by check-arch)

```
apps/web        --> pilcrow-web
apps/backend    --> pilcrow-core
apps/web        -/-> apps/backend (never)
apps/backend    -/-> pilcrow-web (never)
templates       -/-> transport DTOs (never — Props != DTO)
```

## Sandbox

- Will live at `sandbox/` when created. Uses Pilcrow like an external consumer.
- Demonstrates the full canonical flow: build.rs, templates, routing, BFF.
- NOT part of the framework. NOT shipped. Exists only to prove the framework works.
- Any demo app belongs here, not in `apps/`.

## Naming

- The framework is **Pilcrow**. The JS runtime is **Silcrow**.
- Rust types: `SilcrowRequest`, `SilcrowEvent`, `SilcrowTarget`, etc.
- Future phase will unify naming to Pilcrow everywhere.

## Edition and Toolchain

- All crates: Rust edition **2024**.
- No `async-trait` crate. Use native `async fn` in traits or `Pin<Box<dyn Future>>`.
- Axum 0.7 (still requires `#[async_trait]` for `FromRequestParts` via its own re-export).

## Known Overexposed APIs (to be tightened in future phase)

None currently. All known issues were resolved in Phases 1–4.

## Phased Improvement Plan

### Phase 1 — Build correctness (DONE)
- [x] silcrow.js moved to tracked `assets/` directory
- [x] `silcrow_js_path()` fixed (content hash, not content)
- [x] pilcrow-macros edition 2024
- [x] `async-trait` removed from api-client crates
- [x] routekit doc header fixed

### Phase 2 — Tighten pilcrow crate (DONE)
- [x] `html()` -> `#[doc(hidden)]` (generated code uses it, not users)
- [x] `respond!()` macro removed — replaced by `json()` + `ResponseExt` chaining
- [x] Leaked internal macros (`__respond_*`) removed
- [x] `pub mod headers` -> `pub(crate) mod headers`
- [x] `serialize_or_null` -> `pub(crate)`
- [x] `Toast.level` -> `ToastLevel` enum (Info, Success, Warning, Error)

### Phase 3 — Tighten routekit (DONE)
- [x] All sub-modules -> `pub(crate)` (codegen, compiler, discovery, path, pipeline, route)
- [x] `#[non_exhaustive]` on `Route` and `RouteMatch`
- [x] Public API: only `compile_to_out_dir`, `watched_source_directories`, `GeneratedPageRoute`, `ParameterConstraint`, `InterceptLevel`, `LayoutOption`
- [x] `Route` fields -> `pub(crate)` (all 27 fields; no external consumers exist)
- [x] `RouteMatch` borrows `&'a Route` instead of cloning; `match_route` returns `Option<RouteMatch<'_>>`

### Phase 4 — Curate pilcrow-web facade (DONE)
- [x] Replace `pub use pilcrow::*` with explicit re-exports
- [x] `pub use axum` -> `#[doc(hidden)]` (axum is peer dependency)

### Phase 5 — Naming consistency
- [ ] Rename `Silcrow*` types -> `Pilcrow*`

### Phase 6 — Tests
- [ ] routekit: pattern parsing, layout resolution, slot expansion, full pipeline
- [ ] sandbox web/backend: transport DTO and mapping tests
- [ ] pilcrow-cli: check-arch pass/fail, scaffold output
- [ ] pilcrow-core: envelope serialization, error conversion

### Phase 7 — API directory support
- [ ] Extend routekit to compile `api/` routes into JSON endpoint handlers
- [ ] File-based routing for `api/` that mirrors `pages/` but returns JSON

## Coding Rules

- No `unwrap()` in library code. Tests and `build.rs` only.
- Prefer `Option`/`Result` chains over `if/else`.
- Pure transforms separated from Axum boundary methods.
- Every commit compiles. Every commit passes clippy.
- `cargo check && cargo clippy --workspace` before any commit.
- Branch: `{type}/{short-description}`. Commits: `type(scope): description`.

## Response Rules for Agents

- Questions -> answer only, no code unless asked.
- Design decisions -> options + recommendation + tradeoff.
- Code changes -> diff only, not full files.
- New features -> API surface first, wait for approval, then implement.
- Never generate code for TODO items unless explicitly asked.
