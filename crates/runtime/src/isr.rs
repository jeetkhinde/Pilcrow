use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::http::HeaderMap;
use axum_extra::extract::CookieJar;

use crate::context::FormMap;

// ── Cache entry ───────────────────────────────────────────────

struct CacheEntry {
    html: String,
    stored_at: Instant,
    ttl_secs: u64,
    revalidating: bool,
    tags: Vec<String>,
}

impl CacheEntry {
    fn age_secs(&self) -> u64 {
        self.stored_at.elapsed().as_secs()
    }

    fn is_fresh(&self) -> bool {
        self.age_secs() < self.ttl_secs
    }
}

// ── CacheState ────────────────────────────────────────────────

/// The result of looking up a key in the ISR cache.
#[derive(Debug)]
pub enum IsrCacheState {
    /// Data is within the `REVALIDATE` window — serve immediately.
    Fresh(String),
    /// Data has exceeded `REVALIDATE` but is within `MAX_STALE` — serve stale, revalidate in background.
    Stale(String),
    /// No cached entry, or the stale age has exceeded `MAX_STALE` — must render synchronously.
    Miss,
}

// ── IsrCache ──────────────────────────────────────────────────

/// In-process ISR cache backed by a `HashMap<String, CacheEntry>`.
///
/// This is the default (`provider = "memory"`) backend. It is single-node and
/// does not persist across process restarts. Use `provider = "redis"` for
/// multi-node deployments.
///
/// `IsrCache` is `Clone` — cloning shares the same underlying storage.
#[derive(Clone, Default)]
pub struct IsrCache(Arc<Mutex<HashMap<String, CacheEntry>>>);

impl IsrCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check the cache state for a given key.
    ///
    /// - `Fresh`: within `ttl_secs`. Return cached HTML, no action needed.
    /// - `Stale`: beyond `ttl_secs` but within `max_stale` (or no cap). Return
    ///   cached HTML and trigger a background revalidation.
    /// - `Miss`: no entry, or stale age exceeds `max_stale`. Block and render.
    pub async fn check(&self, key: &str, max_stale: Option<u64>) -> IsrCacheState {
        let map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match map.get(key) {
            None => IsrCacheState::Miss,
            Some(entry) => {
                if entry.is_fresh() {
                    IsrCacheState::Fresh(entry.html.clone())
                } else {
                    let stale_secs = entry.age_secs().saturating_sub(entry.ttl_secs);
                    if max_stale.map_or(true, |ms| stale_secs <= ms) {
                        IsrCacheState::Stale(entry.html.clone())
                    } else {
                        IsrCacheState::Miss
                    }
                }
            }
        }
    }

    /// Attempt to set the `revalidating` flag on a cache key.
    ///
    /// Returns `true` if this caller should spawn the revalidation task.
    /// Returns `false` if another concurrent request has already claimed it
    /// (thundering-herd coalescing).
    pub async fn begin_revalidation(&self, key: &str) -> bool {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match map.get_mut(key) {
            None => true,
            Some(entry) => {
                if entry.revalidating {
                    false
                } else {
                    entry.revalidating = true;
                    true
                }
            }
        }
    }

    /// Clear the `revalidating` flag. Always called at the end of a revalidation task,
    /// whether successful or not, so the next stale request can try again.
    pub async fn end_revalidation(&self, key: &str) {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = map.get_mut(key) {
            entry.revalidating = false;
        }
    }

    /// Write a rendered HTML string to the cache with the given TTL and tags.
    pub async fn store(&self, key: &str, html: String, ttl_secs: u64, tags: Vec<String>) {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(
            key.to_string(),
            CacheEntry {
                html,
                stored_at: Instant::now(),
                ttl_secs,
                revalidating: false,
                tags,
            },
        );
    }

    /// Remove all cache entries whose key starts with `path`.
    pub fn invalidate_path(&self, path: &str) {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        map.retain(|k, _| !k.starts_with(path));
    }

    /// Remove all cache entries that carry the given tag.
    pub fn invalidate_tag(&self, tag: &str) {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        map.retain(|_, v| !v.tags.iter().any(|t| t == tag));
    }
}

// ── IsrHandle ─────────────────────────────────────────────────

/// Per-request ISR handle attached to `req.cache`.
///
/// Provides the public invalidation API (`revalidate`, `revalidate_tag`) and
/// the internal accessors used by generated handler code.
#[derive(Clone, Default)]
pub struct IsrHandle {
    cache: Option<Arc<IsrCache>>,
}

impl IsrHandle {
    pub(crate) fn new(cache: Arc<IsrCache>) -> Self {
        Self { cache: Some(cache) }
    }

    /// Invalidate all cached entries whose URL path starts with `path`.
    ///
    /// ```rust,ignore
    /// pub async fn update_product(req: Req) -> ActionResult {
    ///     db.update_product(&req.form).await?;
    ///     req.cache.revalidate("/products/1");
    ///     redirect("/products")
    /// }
    /// ```
    pub fn revalidate(&self, path: &str) {
        if let Some(cache) = &self.cache {
            cache.invalidate_path(path);
        }
    }

    /// Invalidate all cached entries tagged with `tag`.
    ///
    /// ```rust,ignore
    /// pub async fn update_product(req: Req) -> ActionResult {
    ///     db.update_product(&req.form).await?;
    ///     req.cache.revalidate_tag("products");
    ///     redirect("/products")
    /// }
    /// ```
    pub fn revalidate_tag(&self, tag: &str) {
        if let Some(cache) = &self.cache {
            cache.invalidate_tag(tag);
        }
    }

    /// Return an `Arc<IsrCache>` for use in `tokio::spawn` tasks.
    #[doc(hidden)]
    pub fn __arc(&self) -> Option<Arc<IsrCache>> {
        self.cache.clone()
    }
}

impl std::fmt::Debug for IsrHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IsrHandle")
            .field("enabled", &self.cache.is_some())
            .finish()
    }
}

// ── Cache key computation ─────────────────────────────────────

/// Compute the ISR cache key: `{path}?{sorted_query}#{vary_hash}`.
///
/// - `path` — normalized request path
/// - `query` — the request query map (params are sorted for stability)
/// - `vary_keys` — names declared in `CACHE_VARY`; each key is resolved as a
///   cookie name first, then as a request header name. Missing values are
///   treated as empty string so they still produce a stable key segment.
/// - `cookies` / `headers` — the request cookies and headers for vary resolution
#[doc(hidden)]
pub fn __isr_cache_key(
    path: &str,
    query: &FormMap,
    vary_keys: &[&str],
    cookies: &CookieJar,
    headers: &HeaderMap,
) -> String {
    let mut pairs: Vec<(String, String)> = query
        .0
        .iter()
        .flat_map(|(k, vs)| vs.iter().map(move |v| (k.clone(), v.clone())))
        .collect();
    pairs.sort();
    let qs = pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");

    let vary = if vary_keys.is_empty() {
        String::new()
    } else {
        vary_keys
            .iter()
            .map(|k| {
                // Cookie takes priority over header (auth state is cookie-based in Pilcrow).
                cookies
                    .get(k)
                    .map(|c| c.value().to_string())
                    .or_else(|| {
                        headers
                            .get(*k)
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_string)
                    })
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(":")
    };

    match (qs.is_empty(), vary.is_empty()) {
        (true, true) => path.to_string(),
        (false, true) => format!("{path}?{qs}"),
        (true, false) => format!("{path}#{vary}"),
        (false, false) => format!("{path}?{qs}#{vary}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_cookies() -> CookieJar {
        CookieJar::default()
    }

    fn empty_headers() -> HeaderMap {
        HeaderMap::new()
    }

    #[test]
    fn cache_key_no_vary_no_query() {
        let key = __isr_cache_key("/products", &FormMap::default(), &[], &empty_cookies(), &empty_headers());
        assert_eq!(key, "/products");
    }

    #[test]
    fn cache_key_query_sorted() {
        let mut q = FormMap::default();
        q.0.insert("z".into(), vec!["1".into()]);
        q.0.insert("a".into(), vec!["2".into()]);
        let key = __isr_cache_key("/p", &q, &[], &empty_cookies(), &empty_headers());
        assert_eq!(key, "/p?a=2&z=1");
    }

    #[test]
    fn cache_key_vary_by_cookie() {
        let jar = CookieJar::default().add(cookie::Cookie::new("session", "abc123"));
        let key = __isr_cache_key("/dash", &FormMap::default(), &["session"], &jar, &empty_headers());
        assert_eq!(key, "/dash#abc123");
    }

    #[test]
    fn cache_key_vary_by_header_fallback() {
        let mut h = HeaderMap::new();
        h.insert("accept-language", "en-US".parse().unwrap());
        let key = __isr_cache_key("/dash", &FormMap::default(), &["accept-language"], &empty_cookies(), &h);
        assert_eq!(key, "/dash#en-US");
    }

    #[test]
    fn cache_key_vary_cookie_takes_priority_over_header() {
        let jar = CookieJar::default().add(cookie::Cookie::new("locale", "fr"));
        let mut h = HeaderMap::new();
        h.insert("locale", "de".parse().unwrap());
        let key = __isr_cache_key("/p", &FormMap::default(), &["locale"], &jar, &h);
        assert_eq!(key, "/p#fr");
    }

    #[test]
    fn cache_key_vary_missing_key_falls_back_to_base_key() {
        // When the vary key is absent from both cookies and headers, the vary
        // segment is empty and the key degrades to the plain path — the
        // unauthenticated case shares the same cache as the no-vary case.
        let key = __isr_cache_key("/p", &FormMap::default(), &["session"], &empty_cookies(), &empty_headers());
        assert_eq!(key, "/p");
    }
}
