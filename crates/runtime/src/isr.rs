use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::HeaderMap;
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};

use crate::context::FormMap;

// ── Cache entry ───────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    /// The cache key this entry belongs to (stored for filesystem round-trips).
    key: String,
    html: String,
    /// Unix timestamp (seconds) when this entry was stored.
    stored_unix: u64,
    ttl_secs: u64,
    #[serde(skip)]
    revalidating: bool,
    tags: Vec<String>,
}

impl CacheEntry {
    fn new(key: String, html: String, ttl_secs: u64, tags: Vec<String>) -> Self {
        Self {
            key,
            html,
            stored_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            ttl_secs,
            revalidating: false,
            tags,
        }
    }

    fn age_secs(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.stored_unix)
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

// ── Snapshot (for /__pilcrow/isr endpoint) ───────────────────

/// Serializable view of one ISR cache entry — returned by `IsrCache::snapshot()`.
#[derive(Debug, Serialize)]
pub struct CacheEntrySnapshot {
    pub key: String,
    pub age_secs: u64,
    pub ttl_secs: u64,
    pub is_fresh: bool,
    pub revalidating: bool,
    pub tags: Vec<String>,
}

// ── IsrCache ──────────────────────────────────────────────────

struct IsrInner {
    map: HashMap<String, CacheEntry>,
    /// When set, entries are persisted to this directory as JSON files.
    persist_dir: Option<PathBuf>,
}

/// In-process ISR cache backed by a `HashMap<String, CacheEntry>`.
///
/// This is the default (`provider = "memory"`) backend. It is single-node and
/// does not persist across process restarts. Set `provider = "filesystem"` and
/// `dir = ".pilcrow-cache"` in `[cache]` in `Pilcrow.toml` for single-node
/// persistence that survives restarts.
///
/// `IsrCache` is `Clone` — cloning shares the same underlying storage.
#[derive(Clone, Default)]
pub struct IsrCache(Arc<Mutex<IsrInner>>);

impl Default for IsrInner {
    fn default() -> Self {
        Self { map: HashMap::new(), persist_dir: None }
    }
}

impl IsrCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a cache that persists entries as JSON files in `dir`.
    ///
    /// Existing files in `dir` are loaded on construction so the cache is
    /// warm after a server restart. Files whose TTL has already expired are
    /// loaded as stale (they will be revalidated on the first request).
    ///
    /// # Pilcrow.toml
    /// ```toml
    /// [cache]
    /// provider = "filesystem"
    /// dir = ".pilcrow-cache"
    /// ```
    pub fn with_persistence(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&dir);

        let mut map = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(raw) = std::fs::read_to_string(&path) {
                        if let Ok(ce) = serde_json::from_str::<CacheEntry>(&raw) {
                            map.insert(ce.key.clone(), ce);
                        }
                    }
                }
            }
        }

        Self(Arc::new(Mutex::new(IsrInner { map, persist_dir: Some(dir) })))
    }

    /// Check the cache state for a given key.
    pub async fn check(&self, key: &str, max_stale: Option<u64>) -> IsrCacheState {
        let inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match inner.map.get(key) {
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
        let mut inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match inner.map.get_mut(key) {
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

    /// Clear the `revalidating` flag. Always called at the end of a revalidation task.
    pub async fn end_revalidation(&self, key: &str) {
        let mut inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = inner.map.get_mut(key) {
            entry.revalidating = false;
        }
    }

    /// Write a rendered HTML string to the cache with the given TTL and tags.
    pub async fn store(&self, key: &str, html: String, ttl_secs: u64, tags: Vec<String>) {
        let mut inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let entry = CacheEntry::new(key.to_string(), html, ttl_secs, tags);
        if let Some(dir) = &inner.persist_dir {
            let filename = format!("{:016x}.json", crc32fast::hash(key.as_bytes()));
            let path = dir.join(filename);
            if let Ok(json) = serde_json::to_string(&entry) {
                let _ = std::fs::write(path, json);
            }
        }
        inner.map.insert(key.to_string(), entry);
    }

    /// Remove all cache entries whose key starts with `path`.
    pub fn invalidate_path(&self, path: &str) {
        let mut inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let persist_dir = inner.persist_dir.clone();
        inner.map.retain(|k, entry| {
            if k.starts_with(path) {
                if let Some(dir) = &persist_dir {
                    let filename = format!("{:016x}.json", crc32fast::hash(k.as_bytes()));
                    let _ = std::fs::remove_file(dir.join(filename));
                }
                false
            } else {
                let _ = entry; // suppress unused variable warning
                true
            }
        });
    }

    /// Remove all cache entries that carry the given tag.
    pub fn invalidate_tag(&self, tag: &str) {
        let mut inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let persist_dir = inner.persist_dir.clone();
        inner.map.retain(|k, entry| {
            if entry.tags.iter().any(|t| t == tag) {
                if let Some(dir) = &persist_dir {
                    let filename = format!("{:016x}.json", crc32fast::hash(k.as_bytes()));
                    let _ = std::fs::remove_file(dir.join(filename));
                }
                false
            } else {
                true
            }
        });
    }

    /// Return all cached entries as `(key, html)` pairs for static file export.
    ///
    /// Used by [`pilcrow_web::export`] to write static HTML files to disk.
    pub fn export_entries(&self) -> Vec<(String, String)> {
        let inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        inner.map.values().map(|e| (e.key.clone(), e.html.clone())).collect()
    }

    /// Return a snapshot of all current cache entries for inspection.
    ///
    /// Used by the `GET /__pilcrow/isr` dev endpoint.
    pub fn snapshot(&self) -> Vec<CacheEntrySnapshot> {
        let inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        inner.map.values().map(|e| CacheEntrySnapshot {
            key: e.key.clone(),
            age_secs: e.age_secs(),
            ttl_secs: e.ttl_secs,
            is_fresh: e.is_fresh(),
            revalidating: e.revalidating,
            tags: e.tags.clone(),
        }).collect()
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
        // segment is empty and the key degrades to the plain path.
        let key = __isr_cache_key("/p", &FormMap::default(), &["session"], &empty_cookies(), &empty_headers());
        assert_eq!(key, "/p");
    }

    #[tokio::test]
    async fn store_and_check_fresh() {
        let cache = IsrCache::new();
        cache.store("/products", "<h1>Products</h1>".into(), 60, vec![]).await;
        match cache.check("/products", None).await {
            IsrCacheState::Fresh(html) => assert!(html.contains("Products")),
            other => panic!("expected Fresh, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn miss_for_unknown_key() {
        let cache = IsrCache::new();
        assert!(matches!(cache.check("/unknown", None).await, IsrCacheState::Miss));
    }

    #[tokio::test]
    async fn snapshot_reflects_stored_entries() {
        let cache = IsrCache::new();
        cache.store("/a", "html".into(), 60, vec!["tag1".into()]).await;
        let snap = cache.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].key, "/a");
        assert_eq!(snap[0].ttl_secs, 60);
        assert!(snap[0].is_fresh);
        assert_eq!(snap[0].tags, vec!["tag1"]);
    }

    #[tokio::test]
    async fn invalidate_path_removes_matching_entries() {
        let cache = IsrCache::new();
        cache.store("/products", "html".into(), 60, vec![]).await;
        cache.store("/products/1", "html".into(), 60, vec![]).await;
        cache.store("/about", "html".into(), 60, vec![]).await;
        cache.invalidate_path("/products");
        assert!(matches!(cache.check("/products", None).await, IsrCacheState::Miss));
        assert!(matches!(cache.check("/products/1", None).await, IsrCacheState::Miss));
        assert!(matches!(cache.check("/about", None).await, IsrCacheState::Fresh(_)));
    }

    #[tokio::test]
    async fn invalidate_tag_removes_tagged_entries() {
        let cache = IsrCache::new();
        cache.store("/p1", "html".into(), 60, vec!["products".into()]).await;
        cache.store("/p2", "html".into(), 60, vec!["products".into()]).await;
        cache.store("/about", "html".into(), 60, vec!["pages".into()]).await;
        cache.invalidate_tag("products");
        assert!(matches!(cache.check("/p1", None).await, IsrCacheState::Miss));
        assert!(matches!(cache.check("/p2", None).await, IsrCacheState::Miss));
        assert!(matches!(cache.check("/about", None).await, IsrCacheState::Fresh(_)));
    }

    #[tokio::test]
    async fn filesystem_persistence_survives_reload() {
        let dir = tempfile::tempdir().unwrap();
        {
            let cache = IsrCache::with_persistence(dir.path());
            cache.store("/products", "<p>data</p>".into(), 3600, vec!["products".into()]).await;
        }
        // Reload from disk.
        let cache2 = IsrCache::with_persistence(dir.path());
        match cache2.check("/products", None).await {
            IsrCacheState::Fresh(html) => assert_eq!(html, "<p>data</p>"),
            other => panic!("expected Fresh after reload, got {other:?}"),
        }
        let snap = cache2.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].tags, vec!["products"]);
    }
}
