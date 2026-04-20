# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Pilcrow

Pilcrow is a Rust full-stack web framework inspired by SvelteKit/Astro. It uses:

- **Axum** as the HTTP server
- **Askama** for compile-time HTML templating
- **silcrow.js** — Always read `crates/runtime/assets/silcrow.js` via MCP tool `silcrow-docs` before writing anything about it.
- A **build.rs** pipeline (`routekit`) that compiles `.html` + `.rs` files into a wired axum `Router` — no manual route registration

The workspace root `Cargo.toml` has the framework crates as `members`, while `tools/cli` and `sandbox` are `exclude`d and must be built separately.

## Build Commands

```bash
# Framework crates (all must pass before touching sandbox)
cargo build -p pilcrow-routekit
cargo build -p pilcrow-runtime
cargo build -p pilcrow-web

# Sandbox web app (exercises the full build pipeline including build.rs codegen)
cargo build --manifest-path sandbox/apps/web/Cargo.toml

# CLI tool (separate workspace)
cargo build --manifest-path tools/cli/Cargo.toml

# Tests (routekit has the most meaningful test suite)
cargo test -p pilcrow-routekit
cargo test -p pilcrow-routekit -- <test_name>   # single test

# Run sandbox app
cargo run --manifest-path sandbox/apps/web/Cargo.toml
```

Config is in `Pilcrow.toml` (walks up from cwd). Defaults: web on `127.0.0.1:3000`, backend on `127.0.0.1:4000`. Env overrides: `PILCROW_WEB_HOST`, `PILCROW_WEB_PORT`, `PILCROW_BACKEND_URL`.

## Crate Map

| Crate | Role |
|---|---|
| `crates/core` | `AppError`, `AppResult`, `PilcrowConfig`, `Meta` — shared primitives with no framework deps |
| `crates/routekit` | Build-time pipeline: discovers `.html`/`.rs` sources, transpiles templates, emits generated Rust (`generated_app.rs`, `generated_routes.rs`, etc.) |
| `crates/runtime` | Runtime extractors (`Req`, `Res`), response builders, SSE, WebSocket, asset serving, middleware (`Next`) |
| `crates/macros` | `#[handler]` proc-macro, `sse!` macro |
| `crates/client` | `PilcrowClient` — typed HTTP client wrapping `reqwest`, used in `load()` to call backend APIs |
| `crates/web` | Thin facade; re-exports everything a `web` app needs under `pilcrow_web::*` |
| `tools/cli` | `pilcrow-cli new <dir>` scaffold + `check-arch` command |

## How the Build Pipeline Works

`build.rs` in a web app calls `routekit::compile_current_crate_sources()`, which:

1. **Discovers** `src/pages/**/*.html`, `src/ui/**/*.html`, and their `.rs` code-behind files
2. **Classifies** special files: `_layout.html`, `_error.html`, `_not_found.html`, `_loading.html`
3. **Reads** `Pilcrow.toml` for fragment directory configuration (`[[fragments]]`)
4. **Transpiles** Askama-dialect HTML → Askama templates in `$OUT_DIR/pilcrow_templates/`
5. **Generates** `$OUT_DIR/generated_app.rs` — a `build_router()` fn wiring all routes
6. **Generates** `$OUT_DIR/generated_routes.rs` and `$OUT_DIR/generated_api_mods.rs`
7. **Detects** `src/middleware.rs` — if present, wraps the router in an axum middleware layer

The `pilcrow_app!()` macro in `main.rs` includes these generated files.

## File Conventions

```
src/
  pages/
    index.html          # Route: GET /
    index.rs            # Code-behind: Props struct, load(), named action fns
    products.html       # Route: GET /products
    products.rs
    _layout.html        # Auto-wraps all sibling/child pages — not a route
    _error.html         # Shown when load() returns Err — not a route
    _not_found.html     # axum fallback — not a route
    _loading.html       # Skeleton shown during navigation — injected as <template>
    [id]/
      index.html        # Route: GET /:id
    [id=integer]/
      index.html        # Route: GET /:id, but only when id passes src/params/integer::match_param
    (admin)/
      _layout.html      # Layout group — wraps children, NOT part of the URL
      dashboard.html    # Route: GET /dashboard  (group name stripped from URL)
  ui/
    Button.html         # Reusable components, imported with {% import %}
  widgets/              # Example fragment dir (configured in Pilcrow.toml)
    user-card.html      # Route: GET /widgets/user-card (no layout wrapping)
    user-card.rs        # Optional code-behind: same as pages (load, actions, etc.)
  api/
    health.rs           # API route: handlers live here, separate from page routes
  params/
    integer.rs          # Param matcher: `pub fn match_param(value: &str) -> bool`
  middleware.rs         # Optional: global request middleware
```

Route parameters use `:param` in axum style. Folder names in brackets (`[id]`) become URL params.

### Param Matchers

`[id=integer]` maps to `src/params/integer.rs`. The file must export:

```rust
pub fn match_param(value: &str) -> bool { ... }
```

The generated handler calls this at request time and returns 404 if it returns `false`. Built-in constraints (`[id:int]`, `[id:uuid]`, etc.) still work alongside external matchers.

### Route Groups

`(group)/` directories are stripped from URLs and module names. They scope `_layout.html`, `_error.html`, and `_loading.html` to their children without affecting routes.

### Fragment Directories

Fragment directories contain URL-accessible HTML partials — like pages but without any layout wrapping. Configure them in `Pilcrow.toml`:

```toml
[[fragments]]
dir = "widgets"          # relative to src/; URL prefix = "widgets" → /widgets/**

[[fragments]]
dir = "ui-blocks"
url = "blocks"           # explicit URL prefix override → /blocks/**
```

- Each `.html` file in the directory becomes a GET route: `src/widgets/user-card.html` → `GET /widgets/user-card`
- Code-behind `.rs` files work exactly like pages (`load()`, named actions, `_error.html`)
- No layout is applied — the response is the raw fragment HTML
- Fragments can import `ui/` components with `import … from "ui/…";`
- Module names are `frag_{url_prefix}_{snake_path}` (e.g. `frag_widgets_user_card`)
- `cargo:rerun-if-changed` is emitted for each fragment dir and for `Pilcrow.toml`

### Per-page Options

Declare in a code-behind file (or `---` frontmatter):

```rust
pub const TRAILING_SLASH: &str = "always"; // "always" | "never" | "ignore"
pub const LAYOUT: &str = "none";           // opt out of all layout wrapping
```

**`TRAILING_SLASH`:**
- `"always"` — redirects `/path` → `/path/` (GET only)
- `"ignore"` — redirects `/path/` → `/path` (normalise to no-slash)
- `"never"` (default) — no extra routes

**`LAYOUT`:**
- `"none"` — strip all auto-layout wrapping; the page renders directly with no `_layout.html` ancestors applied and no layout `load()` calls

Both constants are stripped from the emitted module and never reach the template.

## Code-Behind Pattern

Each page has two files: `products.html` (template) and `products.rs` (logic).

`products.rs` exports:
- `pub struct Props { ... }` — passed to the template
- `pub async fn load(req: Req) -> AppResult<Props>` — GET handler (**required** if `.rs` file exists)
- `pub async fn <name>(req: Req) -> ActionResult` — one fn per named action. The URL fragment `?/<name>` dispatches to `fn <name>` (POST only). Any number of action fns may be defined; none are required.

**`load()` signature is enforced by the build pipeline.** Any `load()` in `src/pages/` (pages and layouts) must be `async`, return `AppResult<Props>`, and take `req: Req`. The build fails with a clear error otherwise. If you don't need the request, use `_req: Req`. If a page needs no dynamic data, omit the `.rs` file entirely — the page is served as a static template.

The framework injects `use pilcrow_web::Req;`, `use pilcrow_web::ActionResult;`, and `use pilcrow_web::redirect;` at the top of code-behind files automatically (unless already imported).

## Key Types

**`Req`** (`FromRequest`, body-consuming) — unified request context for both `load()` and action fns:
- `.params: HashMap<String, String>` — URL path params (`/posts/:id`)
- `.query: FormMap` — query string as a multi-value map (`?tag=a&tag=b` → `.get_all("tag") == ["a","b"]`). The `?/<name>` action marker is stripped.
- `.form: FormMap` — URL-encoded form body (empty on GET)
- `.cookies: CookieJar`
- `.headers: HeaderMap`
- `.path: String` — e.g. `/products`
- `.is_enhanced: bool` — `true` when request came from silcrow.js (has `silcrow-target` header)
- `.locals: Locals` — per-request typed store shared across all loads in the request
- `.res: Res` — response modifier: set headers, cookies, toasts from inside any handler

**`Req` methods:**
- `req.action() -> &str` — the name of the current action (from the `?/<name>` URL fragment). Returns `""` for plain POSTs with no action. Rarely needed in app code — the router already dispatches to the named fn.
- `req.fail(FormErrors) -> ActionResult` — JSON if enhanced, flash cookie + redirect if plain POST
- `req.take_form_flash() -> Option<FormErrors>` — reads and clears the `silcrow_form_flash` cookie

**`Locals`** — `Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>`:
- `.set<T>(value)` — store a value; `.get<T>() -> Option<T>` — retrieve a clone
- `.require<T>() -> Result<T, AppError>` — get or `Err(AppError::Unauthorized)`
- `.has<T>() -> bool`

**`Res`** — response modifier accessed as `req.res`:
- `.with_status(StatusCode)`, `.with_header(key, value)`, `.with_cookie(Cookie)`
- `.no_cache()`, `.with_toast(msg, ToastLevel::*)`
- `.trigger_event(name)`, `.retarget(selector)`, `.push_history(url)`
- `.patch_target(selector, &data)`, `.invalidate_target(selector)`
- `.client_navigate(path)`, `.sse(path)`, `.ws(path)`
- `.trigger_event`, `.patch_target`, and `.invalidate_target` accumulate across calls (entries are carried in a single JSON-array header, applied in call order). The others overwrite.

**`FormMap`** — multi-value map used for both `req.form` and `req.query`. `.get(key) -> Option<&str>` (first value), `.get_all(key) -> &[String]`, `.contains(key)`, `.keys() -> impl Iterator<Item = &str>`.

**`ActionResult`** — `Result<Response, AppError>` — return type for action fns

**`Deferred<T>`** — wraps a future whose value is streamed to the client after the shell renders:

```rust
pub struct Props {
    pub title: String,
    pub count: Deferred<i32>,   // resolved after shell
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        title: "Hello".to_string(),
        count: Deferred::spawn(async {
            // heavy work here
            42
        }),
    })
}
```

The framework:
1. Renders the shell immediately (`Deferred` fields display as `""` via `Display`).
2. Streams `<script>window.__pilcrow_deferred('count', 42)</script>` as each future resolves.
3. silcrow.js applies the patch via its normal `:text`/`:value` binding system.

`Deferred::ready(value)` creates an already-resolved value (no streaming overhead).

## Response Builders (from `pilcrow_web::*`)

| Builder | Use |
|---|---|
| `redirect("/path")` | `ActionResult`: 303 redirect (use in action fns) |
| `navigate("/path")` | `NavigateResponse`: 303 redirect with `ResponseExt` |
| `json(value)` | JSON response |
| `status(StatusCode::...)` | Bare status |
| `form_errors().error("field", "msg").value("field", val)` | In-place form patching via silcrow.js |
| `AppError::Redirect("/path")` | Redirect from `load()` before render |

`navigate`, `json`, `status`, and `form_errors` implement `ResponseExt`, giving: `.with_toast(msg, ToastLevel::*)`, `.with_header(key, val)`, `.with_status(code)`, `.no_cache()`, `.trigger_event(name)`, `.retarget(sel)`, `.push_history(url)`, `.patch_target(sel, &data)`, `.invalidate_target(sel)`, `.client_navigate(path)`, `.sse(path)`, `.ws(path)`.

## Actions Pattern

Each named action is its own `pub async fn <name>(req: Req) -> ActionResult`. The URL fragment `?/<name>` maps 1:1 to the function named `<name>` in the code-behind file.

```rust
// src/pages/items.rs
pub async fn create(req: Req) -> ActionResult {
    let name = req.form.get("name").unwrap_or("");
    // ...
    redirect("/items")
}

pub async fn delete(req: Req) -> ActionResult {
    // ...
    redirect("/items")
}
```

```html
<form s-post="?/create" s-target="#items">...</form>
<button s-post="?/delete">Delete</button>
```

**Rules:**
- POST only. There is no default action — a plain `POST /items` with no `?/<name>` fragment hits no action route and returns 404.
- Unknown action names (e.g. `?/nonexistent`) return `AppError::NotFound` via the page's `_error.html`.
- Action fns are discovered at build time by signature: `pub async fn <name>(req: Req) -> ActionResult` (or any `Result`-shaped return). `load()` is excluded. Layouts cannot define actions; UI components must not define them (build error).
- Action fns run on the page's own URL — no separate route is registered.

## Form Validation

```rust
pub async fn signup(req: Req) -> ActionResult {
    let email = req.form.get("email").unwrap_or("");
    if email.is_empty() {
        return req.fail(form_errors()
            .error("email", "Email is required")
            .value("email", email));
    }
    redirect("/dashboard")
}

// In load(), repopulate after a plain-browser POST failure:
pub async fn load(req: Req) -> AppResult<Props> {
    let flash = req.take_form_flash();
    Ok(Props { errors: flash, ..Default::default() })
}
```

- **Enhanced** (`req.is_enhanced == true`): `req.fail()` returns JSON that silcrow.js patches into the form
- **Plain browser POST**: `req.fail()` stores errors in a `silcrow_form_flash` cookie and redirects back

## Middleware

Place `src/middleware.rs` at the source root. The build pipeline detects it and wraps the entire axum router automatically:

```rust
// src/middleware.rs
use pilcrow_web::{AppError, Next, Req, Response};
use axum::response::IntoResponse;

pub async fn middleware(req: Req, next: Next) -> Response {
    let token = req.cookies.get("session").map(|c| c.value().to_string());
    match auth::verify(token).await {
        Ok(user) => req.locals.set(user),
        Err(_) if req.path.starts_with("/admin") => {
            return AppError::Unauthorized.into_response();
        }
        _ => {}
    }
    next.run().await
}
```

- `req.locals` and `req.res` set in middleware are shared with all downstream `load()` and action fns
- `next.run().await` forwards the original request (body intact) to the route handler
- Short-circuit by returning a `Response` directly without calling `next.run()`
- `src/middleware.rs` is auto-added to Cargo's rerun-if-changed watch list

## AppError

```rust
AppError::NotFound(String)   // 404
AppError::Unauthorized       // 401
AppError::Validation(String) // 422
AppError::Internal           // 500
AppError::Redirect(String)   // 303 — use from load() to redirect before render
```

`AppError::Redirect` short-circuits before any error page render in the generated handler. Any `req.res` modifiers set before returning `Err(AppError::Redirect(...))` — such as `req.res.with_toast(...)` — are applied to the redirect response.

## silcrow.js

> **Always read `crates/runtime/assets/silcrow.js` (or use MCP `silcrow-docs`) before writing anything about silcrow's API. Do not infer its behaviour from memory.**

### HTTP verb attributes

Elements get one of: `s-get`, `s-post`, `s-put`, `s-patch`, `s-delete` with a URL value.

```html
<a s-get="/products?category=shoes" s-target="#product-list">Shoes</a>
<form s-post="?/create" s-target="#my-form" id="my-form">...</form>
<button s-delete="/items/:key">Delete</button>   <!-- :key interpolated from nearest [:key] ancestor -->
```

### Modifier attributes

| Attribute | Effect |
|---|---|
| `s-target="#sel"` | CSS selector for the swap target (default: the element itself) |
| `s-html` | Request `text/html` instead of `application/json` |
| `s-skip-history` | Don't push to browser history |
| `s-preload` | Prefetch on mouseenter |
| `s-timeout="5000"` | Override request timeout (ms) |
| `s-sse="/path"` | Open SSE connection to URL, patch target on message |
| `s-ws="/path"` | Open WebSocket connection |
| `s-debug` (on `<body>`) | Enable silcrow debug warnings |

### Reactive bindings (colon directives)

```html
<span :text="errors.email"></span>
<span :show="has_errors"></span>
<input :value="values.email" />
<div :class="{ active: is_active }"></div>
<div :style="{ color: label_color }"></div>
<input :disabled="is_loading" />
<div s-use="ui"></div>   <!-- spread: apply all keys from ui object -->
```

### List reconciliation

```html
<template s-for="item in products" :key="item.id">
  <div><span :text="item.title"></span></div>
</template>
```

### Response handling

- JSON response → `Silcrow.patch(data, targetEl)` — applies colon bindings in-place
- HTML response (when `s-html` is set or server returns `text/html`) → `safeSetHTML(targetEl, html)`
- `FormErrors` shape (`{ errors, values, has_errors, error_list }`) patches directly into a form target

### Loading state

While a request is in-flight, silcrow adds `class="silcrow-loading"` and `aria-busy="true"` to the target element.

### Response headers (server-side)

Set these from `req.res` methods to trigger client-side side-effects:

| Header | Effect |
|---|---|
| `silcrow-navigate` | Client-side redirect |
| `silcrow-patch` | Patch a secondary target (`{target, data}`) |
| `silcrow-invalidate` | Clear binding cache for a selector |
| `silcrow-trigger` | Dispatch a custom DOM event |
| `silcrow-retarget` | Override swap target |
| `silcrow-push` | Override history URL |
| `silcrow-sse` | Open SSE connection on target |
| `silcrow-ws` | Open WS connection on target |
| `silcrow-cache: no-cache` | Prevent GET response caching |

### JS API

```js
Silcrow.go(path, {method, body, target, skipHistory})
Silcrow.patch(data, root)
Silcrow.invalidate(root)
Silcrow.stream(root)          // batched high-frequency updates
Silcrow.live(root, url)       // open SSE connection
Silcrow.send(data, root)      // send WS message
Silcrow.disconnect(root)
Silcrow.reconnect(root)
Silcrow.optimistic(data, root)
Silcrow.revert(root)
Silcrow.onToast(handler)
Silcrow.use(middlewareFn)     // patch middleware (must call before DOMContentLoaded)
Silcrow.onRoute(handler)
Silcrow.onError(handler)
```

## routekit Codegen (crates/routekit/src/templating/codegen.rs)

This is the most complex file. Key structs:
- `LoadSignature` — tracks whether `load()` exists, is async, returns Result, wants `Req`, wants `PilcrowClient`. All page/layout load functions are validated to have `is_async=true`, `returns_result=true`, `wants_req=true` — the flexible fields exist for internal tracking but non-conforming signatures are rejected at build time in `instrument_frontmatter`.
- `ActionFn` — one entry per discovered action: `name` (the URL key), `is_async`, `returns_result`, `wants_req`. `action_map: HashMap<page, Vec<ActionFn>>` drives `emit_action_route`, which emits one POST route per page with an inner `match req.action()` dispatch to the named fns (unknown names → `AppError::NotFound`).
- `InstrumentedFrontmatter` — parsed code-behind with injected imports, detected signatures
- `GeneratedTemplatesModule` — all per-page codegen state including `action_map`
- `make_merged_props_struct` — builds `__MergedProps` (layout fields + page fields) when a layout has `load()`. Detects field name collisions between layout and page `Props` at build time and fails with a clear message (e.g. `Props field 'title' is defined in both the layout and the page`).

When editing codegen, always run `cargo test -p pilcrow-routekit` — the pipeline tests exercise the full compile path including codegen.
