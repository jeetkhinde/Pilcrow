use serde::{Deserialize, Serialize};

/// A domain-owned key that links a baked slot to the data that can update it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct DependencyKey(String);

impl DependencyKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DependencyKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DependencyKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BakedSlotKind {
    Text,
    TrustedHtml,
}

impl BakedSlotKind {
    pub fn marker_kind(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::TrustedHtml => "html",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BakedSlot {
    pub name: String,
    pub kind: BakedSlotKind,
    pub dependency_keys: Vec<DependencyKey>,
}

impl BakedSlot {
    pub fn text(name: impl Into<String>, dependency_keys: Vec<DependencyKey>) -> Self {
        Self {
            name: name.into(),
            kind: BakedSlotKind::Text,
            dependency_keys,
        }
    }

    pub fn trusted_html(name: impl Into<String>, dependency_keys: Vec<DependencyKey>) -> Self {
        Self {
            name: name.into(),
            kind: BakedSlotKind::TrustedHtml,
            dependency_keys,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BakeEligibility {
    BuildTime,
    LazyOnFirstHit,
    NeverBake,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BakedArtifactMode {
    #[default]
    FullPage,
    FragmentComposed,
}

/// Developer-facing baked route declaration.
///
/// A declaration chooses when a page may be baked and how its artifact is
/// stored. Actual rendering still belongs to the existing SSR/load pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BakedRouteDeclaration {
    pub route_pattern: String,
    pub concrete_path: String,
    pub eligibility: BakeEligibility,
    #[serde(default)]
    pub artifact_mode: BakedArtifactMode,
    #[serde(default)]
    pub layout_key: Option<String>,
    #[serde(default)]
    pub slots: Vec<BakedSlot>,
}

impl BakedRouteDeclaration {
    pub fn new(
        route_pattern: impl Into<String>,
        concrete_path: impl Into<String>,
        eligibility: BakeEligibility,
    ) -> Self {
        Self {
            route_pattern: route_pattern.into(),
            concrete_path: concrete_path.into(),
            eligibility,
            artifact_mode: BakedArtifactMode::FullPage,
            layout_key: None,
            slots: Vec::new(),
        }
    }

    pub fn lazy_on_first_hit(
        route_pattern: impl Into<String>,
        concrete_path: impl Into<String>,
    ) -> Self {
        Self::new(
            route_pattern,
            concrete_path,
            BakeEligibility::LazyOnFirstHit,
        )
    }

    pub fn build_time(route_pattern: impl Into<String>, concrete_path: impl Into<String>) -> Self {
        Self::new(route_pattern, concrete_path, BakeEligibility::BuildTime)
    }

    pub fn never_bake(route_pattern: impl Into<String>, concrete_path: impl Into<String>) -> Self {
        Self::new(route_pattern, concrete_path, BakeEligibility::NeverBake)
    }

    pub fn full_page(mut self) -> Self {
        self.artifact_mode = BakedArtifactMode::FullPage;
        self.layout_key = None;
        self
    }

    pub fn fragment_composed(mut self, layout_key: impl Into<String>) -> Self {
        self.artifact_mode = BakedArtifactMode::FragmentComposed;
        self.layout_key = Some(layout_key.into());
        self
    }

    pub fn text_slot(
        mut self,
        name: impl Into<String>,
        dependency_keys: Vec<DependencyKey>,
    ) -> Self {
        self.slots.push(BakedSlot::text(name, dependency_keys));
        self
    }

    pub fn trusted_html_slot(
        mut self,
        name: impl Into<String>,
        dependency_keys: Vec<DependencyKey>,
    ) -> Self {
        self.slots
            .push(BakedSlot::trusted_html(name, dependency_keys));
        self
    }

    pub fn slot(&self, name: &str) -> Option<&BakedSlot> {
        self.slots.iter().find(|slot| slot.name == name)
    }

    pub fn dependency_keys(&self) -> Vec<DependencyKey> {
        collect_dependency_keys(&self.slots)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaleState {
    pub stale: bool,
    pub reason: Option<String>,
}

impl StaleState {
    pub fn fresh() -> Self {
        Self {
            stale: false,
            reason: None,
        }
    }

    pub fn stale(reason: impl Into<String>) -> Self {
        Self {
            stale: true,
            reason: Some(reason.into()),
        }
    }
}

/// Metadata for a baked page artifact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BakedPage {
    pub route_pattern: String,
    pub concrete_path: String,
    #[serde(default)]
    pub artifact_mode: BakedArtifactMode,
    pub html_path: String,
    #[serde(default)]
    pub body_path: String,
    pub metadata_path: String,
    #[serde(default)]
    pub layout_key: Option<String>,
    #[serde(default)]
    pub slots: Vec<BakedSlot>,
    #[serde(default)]
    pub dependency_keys: Vec<DependencyKey>,
    pub baked_at: u64,
    pub last_accessed_at: u64,
    pub render_load_version: String,
    pub stale_state: StaleState,
}

impl BakedPage {
    pub fn full_page(
        route_pattern: impl Into<String>,
        concrete_path: impl Into<String>,
        html_path: impl Into<String>,
        metadata_path: impl Into<String>,
        slots: Vec<BakedSlot>,
        baked_at: u64,
        render_load_version: impl Into<String>,
    ) -> Self {
        let html_path = html_path.into();
        Self {
            route_pattern: route_pattern.into(),
            concrete_path: concrete_path.into(),
            artifact_mode: BakedArtifactMode::FullPage,
            html_path: html_path.clone(),
            body_path: html_path,
            metadata_path: metadata_path.into(),
            layout_key: None,
            dependency_keys: collect_dependency_keys(&slots),
            slots,
            baked_at,
            last_accessed_at: baked_at,
            render_load_version: render_load_version.into(),
            stale_state: StaleState::fresh(),
        }
    }

    pub fn fragment_composed(
        route_pattern: impl Into<String>,
        concrete_path: impl Into<String>,
        layout_key: impl Into<String>,
        body_path: impl Into<String>,
        metadata_path: impl Into<String>,
        slots: Vec<BakedSlot>,
        baked_at: u64,
        render_load_version: impl Into<String>,
    ) -> Self {
        let body_path = body_path.into();
        Self {
            route_pattern: route_pattern.into(),
            concrete_path: concrete_path.into(),
            artifact_mode: BakedArtifactMode::FragmentComposed,
            html_path: body_path.clone(),
            body_path,
            metadata_path: metadata_path.into(),
            layout_key: Some(layout_key.into()),
            dependency_keys: collect_dependency_keys(&slots),
            slots,
            baked_at,
            last_accessed_at: baked_at,
            render_load_version: render_load_version.into(),
            stale_state: StaleState::fresh(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BakedLayout {
    pub key: String,
    pub artifact_path: String,
    #[serde(default)]
    pub slots: Vec<BakedSlot>,
    pub version_hash: String,
    pub baked_at: u64,
}

impl BakedLayout {
    pub fn new(
        key: impl Into<String>,
        artifact_path: impl Into<String>,
        slots: Vec<BakedSlot>,
        version_hash: impl Into<String>,
        baked_at: u64,
    ) -> Self {
        Self {
            key: key.into(),
            artifact_path: artifact_path.into(),
            slots,
            version_hash: version_hash.into(),
            baked_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BakedFragment {
    pub key: String,
    pub artifact_path: String,
    #[serde(default)]
    pub slots: Vec<BakedSlot>,
    pub version_hash: String,
    pub baked_at: u64,
}

impl BakedFragment {
    pub fn new(
        key: impl Into<String>,
        artifact_path: impl Into<String>,
        slots: Vec<BakedSlot>,
        version_hash: impl Into<String>,
        baked_at: u64,
    ) -> Self {
        Self {
            key: key.into(),
            artifact_path: artifact_path.into(),
            slots,
            version_hash: version_hash.into(),
            baked_at,
        }
    }
}

fn collect_dependency_keys(slots: &[BakedSlot]) -> Vec<DependencyKey> {
    let mut dependency_keys = Vec::new();
    for slot in slots {
        for key in &slot.dependency_keys {
            if !dependency_keys.iter().any(|existing| existing == key) {
                dependency_keys.push(key.clone());
            }
        }
    }
    dependency_keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_defaults_to_full_page() {
        let declaration = BakedRouteDeclaration::lazy_on_first_hit("/tickets/:id", "/tickets/123");

        assert_eq!(declaration.eligibility, BakeEligibility::LazyOnFirstHit);
        assert_eq!(declaration.artifact_mode, BakedArtifactMode::FullPage);
        assert_eq!(declaration.layout_key, None);
    }

    #[test]
    fn declaration_can_make_full_page_explicit() {
        let declaration = BakedRouteDeclaration::build_time("/about", "/about").full_page();

        assert_eq!(declaration.eligibility, BakeEligibility::BuildTime);
        assert_eq!(declaration.artifact_mode, BakedArtifactMode::FullPage);
        assert_eq!(declaration.layout_key, None);
    }

    #[test]
    fn declaration_can_select_fragment_composed_layout() {
        let declaration = BakedRouteDeclaration::lazy_on_first_hit("/tickets/:id", "/tickets/123")
            .fragment_composed("app");

        assert_eq!(
            declaration.artifact_mode,
            BakedArtifactMode::FragmentComposed
        );
        assert_eq!(declaration.layout_key.as_deref(), Some("app"));
    }

    #[test]
    fn full_page_clears_layout_key() {
        let declaration = BakedRouteDeclaration::build_time("/tickets/:id", "/tickets/123")
            .fragment_composed("app")
            .full_page();

        assert_eq!(declaration.artifact_mode, BakedArtifactMode::FullPage);
        assert_eq!(declaration.layout_key, None);
    }

    #[test]
    fn declaration_collects_unique_dependency_keys_in_slot_order() {
        let ticket = DependencyKey::new("ticket:123");
        let status = DependencyKey::new("ticket:123:status");
        let declaration = BakedRouteDeclaration::lazy_on_first_hit("/tickets/:id", "/tickets/123")
            .text_slot("title", vec![ticket.clone()])
            .trusted_html_slot("status", vec![status.clone(), ticket.clone()]);

        assert_eq!(declaration.dependency_keys(), vec![ticket, status]);
        assert_eq!(
            declaration.slot("status").map(|slot| &slot.kind),
            Some(&BakedSlotKind::TrustedHtml)
        );
    }

    #[test]
    fn page_metadata_collects_dependency_keys() {
        let page = BakedPage::fragment_composed(
            "/tickets/:id",
            "/tickets/123",
            "app",
            ".pilcrow-baked/pages/tickets/[id]/body.html",
            ".pilcrow-baked/pages/tickets/[id]/metadata.json",
            vec![BakedSlot::text(
                "ticket_status",
                vec![DependencyKey::new("ticket:123")],
            )],
            42,
            "render-v1",
        );

        assert_eq!(page.artifact_mode, BakedArtifactMode::FragmentComposed);
        assert_eq!(page.layout_key.as_deref(), Some("app"));
        assert_eq!(page.dependency_keys[0].as_str(), "ticket:123");
        assert_eq!(page.baked_at, 42);
        assert_eq!(page.last_accessed_at, 42);
    }

    #[test]
    fn never_bake_declaration_preserves_refusal_policy() {
        let declaration = BakedRouteDeclaration::never_bake("/account", "/account")
            .text_slot("name", vec![DependencyKey::new("user:1")]);

        assert_eq!(declaration.eligibility, BakeEligibility::NeverBake);
        assert_eq!(declaration.artifact_mode, BakedArtifactMode::FullPage);
        assert_eq!(declaration.dependency_keys()[0].as_str(), "user:1");
    }
}
