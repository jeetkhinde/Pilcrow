/// Per-page options parsed from `pub const` declarations in code-behind files.
///
/// ```rust,ignore
/// pub const TRAILING_SLASH: &str = "always"; // "always" | "never" | "ignore"
/// pub const LAYOUT: &str = "none";           // opt out of all layout wrapping
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageOptions {
    pub trailing_slash: TrailingSlash,
    pub layout: LayoutOpt,
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
