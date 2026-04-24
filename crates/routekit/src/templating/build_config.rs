use std::path::{Path, PathBuf};

/// Build-time configuration read from `Pilcrow.toml` in the crate root.
/// Only fields relevant to the build pipeline are included here.
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct PilcrowBuildConfig {
    /// URL-accessible fragment groups (Option A: flat array, dir name → URL prefix).
    #[serde(default)]
    pub fragments: Vec<FragmentEntry>,

    /// Environment variable declarations — generates typed `env::Public` / `env::Private` structs.
    #[serde(default)]
    pub env: EnvConfig,

    /// Routing configuration (e.g. directories to ignore).
    #[serde(default)]
    pub routing: RoutingConfig,
}

/// Routing configuration.
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct RoutingConfig {
    /// Directory names to ignore during route discovery.
    #[serde(default)]
    pub ignore_directories: Vec<String>,
}

/// Typed environment variable declarations.
///
/// ```toml
/// [env]
/// public  = ["PUBLIC_API_URL", "PUBLIC_APP_NAME"]
/// private = ["DATABASE_URL", "SECRET_KEY"]
/// ```
///
/// The build pipeline generates `pub mod env` with `Public` and `Private` structs, each
/// having typed `String` fields and a `load() -> Result<Self, std::env::VarError>` constructor.
/// Public var field names strip the `PUBLIC_` prefix; private keep the full snake-cased name.
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct EnvConfig {
    /// Vars intended for client-visible use.  `PUBLIC_API_URL` → field `api_url`.
    #[serde(default)]
    pub public: Vec<String>,
    /// Server-only vars.  `DATABASE_URL` → field `database_url`.
    #[serde(default)]
    pub private: Vec<String>,
}

impl EnvConfig {
    /// Convert an env var name to a Rust field name.
    /// Public vars have their `PUBLIC_` prefix stripped before conversion.
    pub fn field_name(var: &str, is_public: bool) -> String {
        let raw = if is_public {
            var.strip_prefix("PUBLIC_").unwrap_or(var)
        } else {
            var
        };
        raw.to_ascii_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect()
    }
}

/// One fragment directory group.
///
/// ```toml
/// [[fragments]]
/// dir = "src/widgets"           # required; relative to crate root
///
/// [[fragments]]
/// dir = "src/ui-blocks"
/// url = "blocks"                # optional; overrides the URL prefix
/// ```
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FragmentEntry {
    /// Directory containing fragment HTML files, relative to the crate root.
    /// e.g. `"src/widgets"` → `src/widgets/**/*.html`
    pub dir: String,

    /// URL prefix for routes generated from this directory.
    /// Defaults to the last path segment of `dir` (e.g. `"widgets"`).
    pub url: Option<String>,
}

impl FragmentEntry {
    /// Resolved URL prefix (no leading slash).
    pub fn url_prefix(&self) -> String {
        self.url.clone().unwrap_or_else(|| {
            self.dir
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or(&self.dir)
                .to_string()
        })
    }

    /// Absolute path of the fragment directory given the crate manifest root.
    pub fn abs_dir(&self, manifest_dir: &Path) -> PathBuf {
        manifest_dir.join(&self.dir)
    }
}

impl PilcrowBuildConfig {
    /// Try to load from `Pilcrow.toml` in `manifest_dir`; silently returns default on missing/parse error.
    pub fn load_from(manifest_dir: &Path) -> Self {
        let path = manifest_dir.join("Pilcrow.toml");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        toml::from_str(&content).unwrap_or_default()
    }
}
