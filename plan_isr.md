# RFC: Live Stale-While-Revalidate (ISR) Architecture for Pilcrow

## Overview
This document outlines the planned architecture for Incremental Static Regeneration (ISR) and Caching in Pilcrow. It specifically details how Pilcrow will bypass traditional static file generation in favor of a robust, zero-latency **Live Stale-While-Revalidate** pattern leveraging standard HTTP streams, Silcrow.js DOM patching, and a persistent Redis cache layer.

## 1. Developer API (Code-Behind)
Developers explicitly opt-in to ISR on a per-route basis using the standard declarative constant pattern in their `index.rs` files.

```rust
// src/pages/products/index.rs
pub const REVALIDATE: u64 = 60; // Route expires in 60 seconds

pub struct Props {
    pub cached_content: String,
    pub fresh_content: DeferredHtml,
}

pub async fn load(req: Req) -> AppResult<Props> {
    // Database and caching logic
}
```

The Routekit codegen pipeline will extract this `REVALIDATE` constant at compile time, removing it from the generated module and wrapping the generated Axum handler with the ISR service middleware.

## 2. The Runtime Workflow (Live Stale-While-Revalidate)
Because Pilcrow runs as a compiled Rust server, we avoid runtime writes to the local filesystem. Instead, we use HTTP Chunked Transfer Encoding coupled with Pilcrow's existing `DeferredHtml` stream capabilities.

When a request arrives for an ISR-enabled route:

1. **Cache Hit (Fresh Data)**
   - The Axum handler checks the cache. The data is less than 60 seconds old.
   - The handler immediately returns the fresh HTML.

2. **Cache Hit (Stale Data)**
   - The data is older than 60 seconds.
   - Axum fetches the stale data from the cache and **instantly streams the initial HTML shell** to the browser (Zero latency).
   - The server **keeps the HTTP connection open**.
   - A `tokio::spawn` task triggers in the background to execute `load()` and fetch the fresh data from the primary database.
   - Once the fresh data is fetched, Pilcrow streams a small Javascript chunk down the open connection (e.g., `<script>window.__pd("target-slot", "<fresh html>")</script>`).
   - The connection is closed.
   - On the client side, Silcrow.js automatically intercepts the script chunk and executes a silent DOM patch, updating the user's screen with the fresh data without requiring a manual browser refresh or dropping their scroll state.

3. **Cache Miss**
   - The cache is entirely empty.
   - Axum blocks, executes `load()`, renders the HTML, writes it to the cache, and serves the response.

## 3. Persistent Cache Layer (Solving the Cold Start)
To prevent the "Thundering Herd" problem when the server boots up (or is redeployed), the ISR cache must survive server restarts. Pilcrow will provide modular cache persistence configured via `Pilcrow.toml`.

### Distributed Caching (Redis)
For robust, horizontally-scaled production deployments, Pilcrow will integrate with Redis.

```toml
# Pilcrow.toml
[cache]
provider = "redis"
url = "redis://..."
```

**Benefits of Redis for Pilcrow ISR:**
- **Horizontal Scalability:** Multiple instances of the Axum binary running behind a load balancer share the exact same cache state. No cache drift.
- **Persistence:** Redis survives Axum application restarts, ensuring the cache is warm on boot.
- **TTL Management:** We rely on native Redis key expiration (`EX 60`) rather than writing custom eviction logic in Rust.

### Local Disk Caching (SQLite Fallback)
For single-node or hobby deployments without a dedicated Redis instance, Pilcrow may offer a local SQLite cache. A background `tokio` task will periodically snapshot the in-memory cache and write it to `.pilcrow/cache.db`. On application startup, the memory cache is hydrated from the SQLite file.

## 4. On-Demand Cache Invalidation
Pilcrow will expose an API to manually bust the cache for specific paths when mutations occur (e.g., inside a named action).

```rust
pub async fn update_product(req: Req) -> ActionResult {
    // ... update database ...
    
    // Purge the specific cache path via the Req context
    req.cache.revalidate("/products/1");
    
    redirect("/products")
}
```

## Summary
By combining Pilcrow's robust streaming capabilities (`DeferredHtml`), Silcrow's seamless DOM patching, and a reliable Redis backing store, Pilcrow will provide a next-generation ISR experience that delivers instant perceived latency while maintaining eventual, real-time consistency.
