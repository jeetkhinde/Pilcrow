# Plan: Incremental Static Regeneration (ISR) for Pilcrow

## Overview

This document specifies the architecture for ISR and caching in Pilcrow. The design goal is a **transparent Live Stale-While-Revalidate** pattern: developers declare `REVALIDATE` and write a normal `load()` — the framework handles caching, streaming, and patching entirely at the middleware layer without touching Props.

---

## 1. Developer API

### Minimal opt-in

```rust
// src/pages/products/index.rs
pub const REVALIDATE: u64 = 60; // seconds

pub struct Props {
    pub products: Vec<Product>,
}

pub async fn load(req: Req) -> AppResult<Props> {
    Ok(Props {
        products: db.fetch_all().await?,
    })
}
```

`REVALIDATE` is the only required constant. The ISR middleware is transparent — `Props` is unchanged, `load()` is unchanged. Routekit extracts `REVALIDATE` at compile time, strips it from the emitted module, and wraps the generated Axum handler with the ISR service.

### Optional per-route constants

```rust
// Scope the cache key by a value from req.locals (e.g., a tenant or user role).
// Without this, all visitors share the same cached HTML.
pub const CACHE_VARY: &[&str] = &["tenant_id"];

// Tag this route for group invalidation.
pub const CACHE_TAGS: &[&str] = &["products", "inventory"];

// If revalidation keeps failing, serve stale content up to this cap (default: indefinite).
pub const MAX_STALE: u64 = 3600;

// Seed the cache at build time (works alongside REVALIDATE).
// Requires the planned SSG feature to be active.
pub const PRERENDER: bool = true;
```

All constants are stripped by Routekit codegen and never reach the template or runtime module.

---

## 2. Runtime Workflow

### Cache key

The cache key is computed as:

```
{route_path}?{sorted_query_string}#{vary_values}
```

- `route_path` — the normalized request path (e.g. `/products/42`)
- `sorted_query_string` — all query params sorted alphabetically to normalize equivalent requests
- `vary_values` — values extracted from `req.locals` for each key listed in `CACHE_VARY`, joined with `:`. Empty string if `CACHE_VARY` is unset.

Example: `/products/42?ref=email&sort=asc` with `CACHE_VARY = &["tenant_id"]` and `tenant_id = "acme"` → key is `/products/42?ref=email&sort=asc#acme`.

### Three-state flow

**1. Cache Hit (Fresh)**
- Data is within the `REVALIDATE` window.
- The handler returns the cached HTML immediately. No DB call, no streaming.

**2. Cache Hit (Stale)**
- Data has exceeded the `REVALIDATE` window but is within `MAX_STALE` (or no cap is set).
- The handler **immediately streams the stale HTML shell** to the browser — zero added latency.
- Before spawning, the handler atomically sets a `revalidating` flag on the cache key. If the flag is already set (another concurrent request triggered revalidation), no second task is spawned (**coalescing** — solves the thundering herd within SWR).
- A single `tokio::spawn` task runs `load()`, renders the fresh HTML, and writes it to the cache.
- Once rendered, the task streams a patch chunk down the still-open connection:
  ```html
  <script>window.__pd_page('<html fragment>')</script>
  ```
- The connection is closed. Silcrow.js intercepts `__pd_page` and performs a silent full-body swap, preserving scroll state and focus.
- The `revalidating` flag is cleared regardless of success or failure.

**3. Cache Miss**
- No cached entry exists, or the stale age has exceeded `MAX_STALE`.
- The handler blocks, runs `load()`, renders the HTML, writes it to the cache, and serves the response synchronously. No streaming.

### Revalidation failure handling

If the background `tokio::spawn` task fails (DB error, timeout, etc.):
- The stale entry remains in place. Its TTL is not reset.
- The error is logged via `tracing::error!`.
- If the stale age exceeds `MAX_STALE`, the route falls back to **Cache Miss** behavior (blocking render) until a successful revalidation writes a fresh entry.

### Revalidation timeout

A global config timeout (default: 30s) caps how long a background task may run before it is aborted and treated as a failure:

```toml
# Pilcrow.toml
[cache]
revalidate_timeout_secs = 30
```

This prevents runaway tasks from holding open HTTP connections indefinitely.

### `window.__pd_page` vs `window.__pd`

`window.__pd(slot, html)` is the existing `DeferredHtml` mechanism for partial slot patches. ISR uses a distinct `window.__pd_page(html)` shim injected once into `<head>`, which replaces the full rendered body rather than a named slot. The two mechanisms coexist and can be active on the same page simultaneously — a page can have both an ISR-cached shell and `DeferredHtml` sub-components streaming in.

---

## 3. Persistent Cache Layer

### Redis (production / multi-node)

```toml
# Pilcrow.toml
[cache]
provider = "redis"
url = "redis://127.0.0.1:6379"
```

- Multiple Axum instances behind a load balancer share a single cache. No drift.
- TTL management via native Redis key expiration (`EX {revalidate}`). No custom eviction logic in Rust.
- Cache survives Axum restarts — no cold-start thundering herd.
- The `revalidating` coalescing flag is a Redis `SETNX` key with TTL `revalidate_timeout_secs`, so it self-clears even if the task crashes before explicitly clearing it.

### SQLite (single-node / local / hobby)

```toml
# Pilcrow.toml
[cache]
provider = "sqlite"             # default when no provider is specified
path = ".pilcrow/cache.db"
```

- Uses WAL mode to prevent corruption on crash mid-write.
- Each cache write is committed immediately — no periodic snapshot — to minimize data loss on unclean shutdown.
- On startup, the in-memory LRU is hydrated from the SQLite file.
- The coalescing flag is an in-process `AtomicBool` per cache key (single-node only; Redis handles this for multi-node via `SETNX`).

---

## 4. Cache Invalidation

### Tag-based invalidation (preferred)

Tag routes with `CACHE_TAGS` and bust entire groups from any action or background job:

```rust
pub async fn update_product(req: Req) -> ActionResult {
    db.update_product(&req.form).await?;
    req.cache.revalidate_tag("products"); // busts every route tagged "products"
    redirect("/products")
}
```

Tag invalidation is preferred over path invalidation because it handles list pages, search results, and any other route that surfaces the mutated data — without the action needing to enumerate every affected URL.

### On-demand path invalidation

```rust
pub async fn update_product(req: Req) -> ActionResult {
    db.update_product(&req.form).await?;
    req.cache.revalidate("/products/1");
    redirect("/products")
}
```

### Per-request cache bypass

For preview mode, admin views, or force-fresh renders:

```rust
pub async fn load(req: Req) -> AppResult<Props> {
    if req.query.contains("preview") {
        req.res.bypass_cache();
    }
    // ...
}
```

`bypass_cache()` forces `load()` to run even on a fresh cache hit and suppresses writing the result back to the cache, so preview renders never pollute the shared cache.

---

## 5. Layout Interaction

When both a layout and a child page define `REVALIDATE`, the **final merged HTML** (layout + page) is cached as a single unit under the child page's cache key. Layout and page are not cached independently.

Rationale: caching them separately would require partial-assembly on every request, adding complexity with little benefit. The merged-unit model is simpler and aligns with how `__MergedProps` works in codegen today.

If a layout should not be cached alongside its children, set `LAYOUT: "none"` on the page to opt out of layout wrapping entirely, then cache the bare page HTML.

---

## 6. Build-Time Pre-Warming (SSG + ISR)

When both `PRERENDER: bool = true` and `REVALIDATE: u64 = N` are set, the build pipeline:

1. Renders the route at build time by calling `load()` with a synthetic request.
2. Pushes the rendered HTML to the configured cache backend with TTL `N`.
3. On deploy, the first request to any pre-warmed route is always a **Cache Hit (Fresh)** — no cold-start miss.

For dynamic routes (`/products/[id]`), pre-warming requires a `prerender_paths()` export:

```rust
pub async fn prerender_paths() -> Vec<HashMap<String, String>> {
    db.all_product_ids().await
        .into_iter()
        .map(|id| HashMap::from([("id".into(), id.to_string())]))
        .collect()
}
```

This mirrors SvelteKit's `entries()` / Astro's `getStaticPaths()` pattern.

---

## 7. Codegen Pipeline Changes

Routekit changes required:

- Detect `REVALIDATE` and optional `CACHE_TAGS`, `CACHE_VARY`, `MAX_STALE`, `PRERENDER` constants in code-behind files.
- Strip all ISR constants from the emitted module (same treatment as `TRAILING_SLASH` and `LAYOUT`).
- Wrap the generated Axum handler in an `IsrLayer` service that receives the extracted constants as static config.
- Emit a `prerender_paths` call site in the build script when `PRERENDER = true`.

ISR constants join the existing stripped-constant set in `instrument_frontmatter`.

---

## 8. Configuration Reference

```toml
# Pilcrow.toml
[cache]
provider = "sqlite"             # "redis" | "sqlite" (default: "sqlite")
url = "redis://127.0.0.1:6379"  # required when provider = "redis"
path = ".pilcrow/cache.db"      # sqlite only
revalidate_timeout_secs = 30    # max background revalidation task duration
```

---

## Summary

| Concern | Approach |
|---|---|
| Developer API | `REVALIDATE` constant only; `Props` and `load()` are unchanged |
| Cache key | path + sorted query string + `CACHE_VARY` locals |
| Thundering herd (SWR) | `SETNX` coalescing flag in Redis; `AtomicBool` for SQLite |
| Stale patch delivery | Chunked TE + `window.__pd_page` (distinct from `DeferredHtml` slot patches) |
| Revalidation failure | Stale persists; error logged; `MAX_STALE` triggers blocking fallback |
| Invalidation | Tag-based (`revalidate_tag`) primary; path-based secondary; per-request bypass |
| Cold start | Redis persistence + optional `PRERENDER` build-time seed |
| Layout caching | Merged HTML unit; layout does not cache independently |
| Multi-node | Redis shared cache; coalescing via `SETNX` with self-expiring TTL |
