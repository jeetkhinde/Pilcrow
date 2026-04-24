/// ISR options parsed from `pub const` declarations in code-behind files.
///
/// All constants are stripped from the emitted module — they never reach runtime code.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IsrOpts {
    /// Cache TTL in seconds (`REVALIDATE`). `None` means ISR is disabled for this page.
    pub revalidate: Option<u64>,
    /// Maximum stale age in seconds before falling back to blocking render (`MAX_STALE`).
    pub max_stale: Option<u64>,
    /// Tag names for group invalidation (`CACHE_TAGS`).
    pub cache_tags: Vec<String>,
    /// Locals keys whose values scope the cache key (`CACHE_VARY`).
    pub cache_vary: Vec<String>,
    /// Whether to pre-warm this route at build time (`PRERENDER`).
    pub prerender: bool,
}

impl IsrOpts {
    /// `true` when this page has ISR enabled (i.e. `REVALIDATE` is set).
    pub fn is_active(&self) -> bool {
        self.revalidate.is_some()
    }
}

/// Per-page options parsed from `pub const` declarations in code-behind files.
///
/// ```rust,ignore
/// pub const TRAILING_SLASH: &str = "always"; // "always" | "never" | "ignore"
/// pub const LAYOUT: &str = "none";           // opt out of all layout wrapping
/// pub const REVALIDATE: u64 = 60;            // ISR: cache TTL in seconds
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageOptions {
    pub trailing_slash: TrailingSlash,
    pub layout: LayoutOpt,
    pub isr: IsrOpts,
}

/// Whether this page participates in the automatic layout chain.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LayoutOpt {
    /// Default — inherit the full auto-layout chain from ancestor `_layout.html` files.
    #[default]
    Inherit,
    /// Strip all layout wrapping: the page renders its own template directly.
    None,
}

/// How the framework handles a trailing slash for this page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TrailingSlash {
    /// Default — no extra routes generated; axum matches the pattern as-is.
    #[default]
    Never,
    /// Redirect `GET /path` → `/path/` when a request arrives without a trailing slash.
    Always,
    /// Redirect `GET /path/` → `/path` when a request arrives with a trailing slash.
    Ignore,
}

impl TrailingSlash {
    pub fn from_str(s: &str) -> Self {
        match s.trim_matches('"').trim_matches('\'') {
            "always" => Self::Always,
            "ignore" => Self::Ignore,
            _ => Self::Never,
        }
    }
}
