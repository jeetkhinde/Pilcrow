use super::{
    BakeEligibility, BakedArtifactMode, BakedPage, BakedPageStore, BakedRenderedPage,
    BakedRouteDeclaration, BakedSlotKind,
};
use std::{
    fs, io,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BakedServeState {
    Hit,
    MissRendered,
    RenderedUnbaked,
}

impl BakedServeState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::MissRendered => "miss-rendered",
            Self::RenderedUnbaked => "never-bake-rendered",
        }
    }

    pub fn render_state(self) -> &'static str {
        match self {
            Self::Hit => "skipped",
            Self::MissRendered | Self::RenderedUnbaked => "ran",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakedServeOutcome {
    pub html: String,
    pub state: BakedServeState,
    pub page: Option<BakedPage>,
}

impl BakedPageStore {
    pub fn get_or_render_declared<F>(
        &self,
        declaration: &BakedRouteDeclaration,
        render: F,
    ) -> io::Result<BakedServeOutcome>
    where
        F: FnOnce(&BakedRouteDeclaration) -> io::Result<BakedRenderedPage>,
    {
        match declaration.eligibility {
            BakeEligibility::LazyOnFirstHit => {
                if let Some(hit) = self.serve_declared_if_fresh(declaration)? {
                    return Ok(hit);
                }

                let rendered = render(declaration)?;
                let page = page_from_declaration(
                    self,
                    declaration,
                    unix_timestamp(),
                    rendered.render_load_version,
                );
                let page = self.write_artifact(&page, &rendered.html)?;
                let html = self.render_page_response(&page, &rendered.html)?;

                Ok(BakedServeOutcome {
                    html,
                    state: BakedServeState::MissRendered,
                    page: Some(page),
                })
            }
            BakeEligibility::BuildTime => {
                if let Some(hit) = self.serve_declared_if_fresh(declaration)? {
                    return Ok(hit);
                }
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "build_time declaration has no fresh prebaked artifact",
                ))
            }
            BakeEligibility::NeverBake => {
                let rendered = render(declaration)?;
                Ok(BakedServeOutcome {
                    html: rendered.html,
                    state: BakedServeState::RenderedUnbaked,
                    page: None,
                })
            }
        }
    }

    pub fn serve_declared_if_fresh(
        &self,
        declaration: &BakedRouteDeclaration,
    ) -> io::Result<Option<BakedServeOutcome>> {
        let Some(hit) = self.serve_if_fresh(&declaration.concrete_path)? else {
            return Ok(None);
        };
        let html = self.render_page_response(&hit.page, &hit.html)?;
        Ok(Some(BakedServeOutcome {
            html,
            state: BakedServeState::Hit,
            page: Some(hit.page),
        }))
    }

    pub fn render_page_response(&self, page: &BakedPage, stored_html: &str) -> io::Result<String> {
        match page.artifact_mode {
            BakedArtifactMode::FullPage => Ok(stored_html.to_string()),
            BakedArtifactMode::FragmentComposed => {
                let layout_key = page.layout_key.as_deref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "fragment-composed page is missing layout key",
                    )
                })?;
                let layout = fs::read_to_string(self.layout_path(layout_key))?;
                replace_slot_region(
                    &layout,
                    "page_body",
                    &BakedSlotKind::TrustedHtml,
                    stored_html,
                )
            }
        }
    }
}

fn replace_slot_region(
    html: &str,
    slot: &str,
    kind: &BakedSlotKind,
    replacement: &str,
) -> io::Result<String> {
    let start = format!(
        "<!--pilcrow-slot:start {slot} kind={}-->",
        kind.marker_kind()
    );
    let end = format!("<!--pilcrow-slot:end {slot}-->");
    let start_pos = single_marker_pos(html, &start)?;
    let end_pos = single_marker_pos(html, &end)?;
    if start_pos >= end_pos {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "slot start marker appears after end marker",
        ));
    }

    let mut composed = String::with_capacity(html.len() + replacement.len());
    composed.push_str(&html[..start_pos]);
    composed.push_str(replacement);
    composed.push_str(&html[end_pos + end.len()..]);
    Ok(composed)
}

fn single_marker_pos(html: &str, marker: &str) -> io::Result<usize> {
    let mut matches = html.match_indices(marker);
    let Some((pos, _)) = matches.next() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing marker {marker}"),
        ));
    };
    if matches.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("duplicate marker {marker}"),
        ));
    }
    Ok(pos)
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
    use crate::baked_pages::DependencyKey;
    use std::{cell::Cell, fs, rc::Rc};

    fn render_counter() -> Rc<Cell<usize>> {
        Rc::new(Cell::new(0))
    }

    fn render_page(
        counter: Rc<Cell<usize>>,
        html: &'static str,
    ) -> impl FnOnce(&BakedRouteDeclaration) -> io::Result<BakedRenderedPage> {
        move |_declaration| {
            counter.set(counter.get() + 1);
            Ok(BakedRenderedPage::new(html, "render-v1"))
        }
    }

    fn page_body_slot(content: &str) -> String {
        format!(
            "<!--pilcrow-slot:start page_body kind=html-->{content}<!--pilcrow-slot:end page_body-->"
        )
    }

    #[test]
    fn lazy_full_page_miss_writes_then_second_request_hits_and_skips_render() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let declaration = BakedRouteDeclaration::lazy_on_first_hit("/example", "/docs/intro")
            .full_page()
            .text_slot("status", vec![DependencyKey::new("docs:intro")]);
        let counter = render_counter();

        let first = store
            .get_or_render_declared(
                &declaration,
                render_page(counter.clone(), "<html>fresh</html>"),
            )
            .unwrap();

        assert_eq!(first.state, BakedServeState::MissRendered);
        assert_eq!(first.html, "<html>fresh</html>");
        assert_eq!(counter.get(), 1);
        assert_eq!(
            fs::read_to_string(store.html_path("/docs/intro")).unwrap(),
            "<html>fresh</html>"
        );
        assert!(store.read_page("/docs/intro").unwrap().is_some());

        let second = store
            .get_or_render_declared(
                &declaration,
                render_page(counter.clone(), "<html>rerendered</html>"),
            )
            .unwrap();

        assert_eq!(second.state, BakedServeState::Hit);
        assert_eq!(second.html, "<html>fresh</html>");
        assert_eq!(counter.get(), 1);
    }

    #[test]
    fn lazy_fragment_composed_writes_body_composes_response_and_skips_second_render() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        fs::create_dir_all(store.layout_path("app").parent().unwrap()).unwrap();
        fs::write(
            store.layout_path("app"),
            format!("<html><body>{}</body></html>", page_body_slot("fallback")),
        )
        .unwrap();
        let declaration = BakedRouteDeclaration::lazy_on_first_hit("/example", "/docs/intro")
            .fragment_composed("app")
            .text_slot("status", vec![DependencyKey::new("docs:intro")]);
        let counter = render_counter();

        let first = store
            .get_or_render_declared(
                &declaration,
                render_page(counter.clone(), "<main>body</main>"),
            )
            .unwrap();

        assert_eq!(first.state, BakedServeState::MissRendered);
        assert_eq!(first.html, "<html><body><main>body</main></body></html>");
        assert_eq!(counter.get(), 1);
        assert_eq!(
            fs::read_to_string(store.body_path("/docs/intro")).unwrap(),
            "<main>body</main>"
        );

        let second = store
            .get_or_render_declared(
                &declaration,
                render_page(counter.clone(), "<main>rerendered</main>"),
            )
            .unwrap();

        assert_eq!(second.state, BakedServeState::Hit);
        assert_eq!(second.html, "<html><body><main>body</main></body></html>");
        assert_eq!(counter.get(), 1);
    }

    #[test]
    fn build_time_serving_path_only_reads_existing_prebaked_artifact() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let declaration = BakedRouteDeclaration::build_time("/example", "/docs/intro").full_page();
        let counter = render_counter();

        let err = store
            .get_or_render_declared(
                &declaration,
                render_page(counter.clone(), "<html>rendered</html>"),
            )
            .unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn never_bake_renders_without_writing_artifact() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let declaration = BakedRouteDeclaration::never_bake("/example", "/docs/intro");
        let counter = render_counter();

        let outcome = store
            .get_or_render_declared(
                &declaration,
                render_page(counter.clone(), "<html>live</html>"),
            )
            .unwrap();

        assert_eq!(outcome.state, BakedServeState::RenderedUnbaked);
        assert_eq!(outcome.html, "<html>live</html>");
        assert_eq!(counter.get(), 1);
        assert!(store.read_page("/docs/intro").unwrap().is_none());
    }
}
