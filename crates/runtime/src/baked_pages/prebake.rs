use super::{BakeEligibility, BakedArtifactMode, BakedPage, BakedPageStore, BakedRouteDeclaration};
use std::{
    io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakedRenderedPage {
    pub html: String,
    pub render_load_version: String,
}

impl BakedRenderedPage {
    pub fn new(html: impl Into<String>, render_load_version: impl Into<String>) -> Self {
        Self {
            html: html.into(),
            render_load_version: render_load_version.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakedPrebakeReport {
    pub page: BakedPage,
    pub artifact_path: PathBuf,
}

impl BakedPageStore {
    pub fn prebake_declared<F>(
        &self,
        declaration: &BakedRouteDeclaration,
        render: F,
    ) -> io::Result<BakedPrebakeReport>
    where
        F: FnOnce(&BakedRouteDeclaration) -> io::Result<BakedRenderedPage>,
    {
        let rendered = render(declaration)?;
        self.prebake_declared_html(declaration, rendered.html, rendered.render_load_version)
    }

    pub fn prebake_declared_html(
        &self,
        declaration: &BakedRouteDeclaration,
        html: impl Into<String>,
        render_load_version: impl Into<String>,
    ) -> io::Result<BakedPrebakeReport> {
        match declaration.eligibility {
            BakeEligibility::BuildTime => {}
            BakeEligibility::LazyOnFirstHit => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "lazy declarations are baked on first request, not during prebake",
                ));
            }
            BakeEligibility::NeverBake => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "never_bake declarations cannot be prebaked",
                ));
            }
        }

        let html = html.into();
        let page = page_from_declaration(
            self,
            declaration,
            unix_timestamp(),
            render_load_version.into(),
        );
        let page = self.write_artifact(&page, &html)?;
        let artifact_path = PathBuf::from(&page.body_path);

        Ok(BakedPrebakeReport {
            page,
            artifact_path,
        })
    }
}

fn page_from_declaration(
    store: &BakedPageStore,
    declaration: &BakedRouteDeclaration,
    baked_at: u64,
    render_load_version: String,
) -> BakedPage {
    match declaration.artifact_mode {
        BakedArtifactMode::FullPage => BakedPage::full_page(
            declaration.route_pattern.clone(),
            declaration.concrete_path.clone(),
            store
                .html_path(&declaration.concrete_path)
                .to_string_lossy()
                .to_string(),
            store
                .metadata_path(&declaration.concrete_path)
                .to_string_lossy()
                .to_string(),
            declaration.slots.clone(),
            baked_at,
            render_load_version,
        ),
        BakedArtifactMode::FragmentComposed => BakedPage::fragment_composed(
            declaration.route_pattern.clone(),
            declaration.concrete_path.clone(),
            declaration.layout_key.clone().unwrap_or_default(),
            store
                .body_path(&declaration.concrete_path)
                .to_string_lossy()
                .to_string(),
            store
                .metadata_path(&declaration.concrete_path)
                .to_string_lossy()
                .to_string(),
            declaration.slots.clone(),
            baked_at,
            render_load_version,
        ),
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baked_pages::{
        text_slot_content, BakedPatchRegistry, BakedSlotKind, DependencyKey, SlotValue,
    };
    use std::fs;

    const DEP: &str = "entity:1";

    fn slot_marker(slot: &str, kind: &BakedSlotKind, content: &str) -> String {
        format!(
            "<!--pilcrow-slot:start {slot} kind={}-->{content}<!--pilcrow-slot:end {slot}-->",
            kind.marker_kind()
        )
    }

    fn full_page_declaration(path: &str) -> BakedRouteDeclaration {
        BakedRouteDeclaration::build_time("/example", path)
            .full_page()
            .text_slot("status", vec![DependencyKey::new(DEP)])
    }

    fn fragment_declaration(path: &str) -> BakedRouteDeclaration {
        BakedRouteDeclaration::build_time("/example", path)
            .fragment_composed("app")
            .text_slot("status", vec![DependencyKey::new(DEP)])
    }

    #[test]
    fn build_time_full_page_prebake_writes_artifact_and_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let declaration = full_page_declaration("/docs/intro");

        let report = store
            .prebake_declared_html(&declaration, "<html>ready</html>", "render-v1")
            .unwrap();

        assert_eq!(
            fs::read_to_string(store.html_path("/docs/intro")).unwrap(),
            "<html>ready</html>"
        );
        assert_eq!(report.page.artifact_mode, BakedArtifactMode::FullPage);
        assert_eq!(report.page.render_load_version, "render-v1");
        assert_eq!(report.artifact_path, store.html_path("/docs/intro"));
    }

    #[test]
    fn build_time_fragment_composed_prebake_writes_body_and_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let declaration = fragment_declaration("/docs/intro");

        let report = store
            .prebake_declared(&declaration, |_declaration| {
                Ok(BakedRenderedPage::new("<main>body</main>", "render-v1"))
            })
            .unwrap();

        assert_eq!(
            fs::read_to_string(store.body_path("/docs/intro")).unwrap(),
            "<main>body</main>"
        );
        assert_eq!(
            report.page.artifact_mode,
            BakedArtifactMode::FragmentComposed
        );
        assert_eq!(report.page.layout_key.as_deref(), Some("app"));
        assert_eq!(report.artifact_path, store.body_path("/docs/intro"));
    }

    #[test]
    fn prebake_updates_reverse_index_from_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let declaration = full_page_declaration("/docs/intro");

        store
            .prebake_declared_html(&declaration, "<html>ready</html>", "render-v1")
            .unwrap();

        let index = store.reverse_index().unwrap().unwrap();
        assert_eq!(index[DEP]["/docs/intro"], vec!["status".to_string()]);
    }

    #[test]
    fn serve_if_fresh_returns_prebaked_full_page() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let declaration = full_page_declaration("/docs/intro");

        store
            .prebake_declared_html(&declaration, "<html>ready</html>", "render-v1")
            .unwrap();

        let hit = store.serve_if_fresh("/docs/intro").unwrap().unwrap();
        assert_eq!(hit.html, "<html>ready</html>");
        assert_eq!(hit.page.artifact_mode, BakedArtifactMode::FullPage);
    }

    #[test]
    fn dependency_patching_works_against_prebaked_full_page() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let declaration = full_page_declaration("/docs/intro");
        store
            .prebake_declared_html(
                &declaration,
                slot_marker("status", &BakedSlotKind::Text, "old"),
                "render-v1",
            )
            .unwrap();
        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_slot_recompute("status", |_key, _page| Ok(SlotValue::text("new")));

        let outcome = registry.patch_dependency(DependencyKey::new(DEP)).unwrap();

        assert_eq!(outcome.patched_pages, vec!["/docs/intro".to_string()]);
        assert_eq!(
            fs::read_to_string(store.html_path("/docs/intro")).unwrap(),
            slot_marker(
                "status",
                &BakedSlotKind::Text,
                &text_slot_content("status", "new")
            )
        );
    }

    #[test]
    fn dependency_patching_works_against_prebaked_fragment_body() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let declaration = fragment_declaration("/docs/intro");
        store
            .prebake_declared_html(
                &declaration,
                slot_marker("status", &BakedSlotKind::Text, "old"),
                "render-v1",
            )
            .unwrap();
        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_slot_recompute("status", |_key, _page| Ok(SlotValue::text("new body")));

        let outcome = registry.patch_dependency(DependencyKey::new(DEP)).unwrap();

        assert_eq!(outcome.patched_pages, vec!["/docs/intro".to_string()]);
        let body = fs::read_to_string(store.body_path("/docs/intro")).unwrap();
        assert!(body.contains("new body"));
    }

    #[test]
    fn never_bake_cannot_be_prebaked() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let declaration = BakedRouteDeclaration::never_bake("/example", "/docs/intro");

        let err = store
            .prebake_declared_html(&declaration, "<html>nope</html>", "render-v1")
            .unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(store.read_page("/docs/intro").unwrap().is_none());
    }
}
