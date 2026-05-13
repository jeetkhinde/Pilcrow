use serde::{Deserialize, Serialize};

/// Typed dependency key. Serialises to `"table:column=value"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyKey {
    pub table: &'static str,
    pub column: &'static str,
    pub value: String,
}

impl DependencyKey {
    pub fn as_dep_string(&self) -> String {
        format!("{}:{}={}", self.table, self.column, self.value)
    }
}

impl std::fmt::Display for DependencyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}={}", self.table, self.column, self.value)
    }
}

/// A field whose value is tracked, cached, and live-patched by Pilcrow FSR.
///
/// `T` must implement `serde::Serialize + serde::de::DeserializeOwned`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveProps<T> {
    pub value: T,
    pub depends_on: Vec<String>, // stored as "table:column=value" strings
    pub promote_after: Option<u32>,
    pub patch_debounce: Option<u32>,
}

impl<T: Serialize + Clone> LiveProps<T> {
    pub fn new(value: T, depends_on: Vec<DependencyKey>) -> Self {
        Self {
            value,
            depends_on: depends_on.iter().map(|d| d.as_dep_string()).collect(),
            promote_after: None,
            patch_debounce: None,
        }
    }

    pub fn promote_after(mut self, hits: u32) -> Self {
        self.promote_after = Some(hits);
        self
    }

    pub fn patch_debounce(mut self, seconds: u32) -> Self {
        self.patch_debounce = Some(seconds);
        self
    }
}
