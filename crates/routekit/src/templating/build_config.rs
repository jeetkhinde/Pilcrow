use std::path::{Path, PathBuf};

/// Build-time configuration read from `Pilcrow.toml` in the crate root.
/// Only fields relevant to the build pipeline are included here.
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct PilcrowBuildConfig {
    /// URL-accessible fragment groups (Option A: flat array, dir name → URL prefix).
    #[serde(default)]
    pub fragments: Vec<FragmentEntry>,
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
