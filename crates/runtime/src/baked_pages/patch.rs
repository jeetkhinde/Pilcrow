use super::{BakedPageStore, BakedRouteDeclaration, BakedSlot, BakedSlotKind, DependencyKey};
use std::{collections::BTreeMap, io};

pub enum SlotValue {
    Text(String),
    TrustedHtml(TrustedHtml),
}

impl SlotValue {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    pub fn trusted_html(value: TrustedHtml) -> Self {
        Self::TrustedHtml(value)
    }

    pub fn render_for_slot(self, slot: &BakedSlot) -> io::Result<String> {
        match (slot.kind.clone(), self) {
            (BakedSlotKind::Text, Self::Text(value)) => Ok(text_slot_content(&slot.name, &value)),
            (BakedSlotKind::TrustedHtml, Self::TrustedHtml(value)) => {
                Ok(trusted_html_slot_content(value))
            }
            (BakedSlotKind::Text, Self::TrustedHtml(_)) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "trusted HTML cannot be written into a text slot",
            )),
            (BakedSlotKind::TrustedHtml, Self::Text(_)) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "text cannot be written into a trusted HTML slot without an explicit wrapper",
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedHtml(String);

impl TrustedHtml {
    pub fn from_sanitized(html: impl Into<String>) -> Self {
        Self(html.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

type RecomputeFn = Box<dyn Fn(&DependencyKey, &str) -> io::Result<SlotValue> + Send + Sync>;

pub struct BakedPatchRegistry {
    store: BakedPageStore,
    recompute_fns: BTreeMap<String, RecomputeFn>,
}

impl BakedPatchRegistry {
    pub fn new(store: BakedPageStore) -> Self {
        Self {
            store,
            recompute_fns: BTreeMap::new(),
        }
    }

    pub fn store(&self) -> &BakedPageStore {
        &self.store
    }

    pub fn register_slot_recompute<F>(&mut self, slot: impl Into<String>, recompute: F)
    where
        F: Fn(&DependencyKey, &str) -> io::Result<SlotValue> + Send + Sync + 'static,
    {
        self.recompute_fns.insert(slot.into(), Box::new(recompute));
    }

    pub fn register_declared_slot_recompute<F>(
        &mut self,
        declaration: &BakedRouteDeclaration,
        slot: &str,
        recompute: F,
    ) -> io::Result<()>
    where
        F: Fn(&DependencyKey, &str) -> io::Result<SlotValue> + Send + Sync + 'static,
    {
        if declaration.slot(slot).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "slot `{slot}` is not declared for route `{}`",
                    declaration.route_pattern
                ),
            ));
        }
        self.register_slot_recompute(slot, recompute);
        Ok(())
    }

    pub fn patch_dependency(&self, key: impl Into<DependencyKey>) -> io::Result<BakedPatchOutcome> {
        let key = key.into();
        let index = self.store.ensure_reverse_index()?;
        let pages = index.get(key.as_str()).cloned().unwrap_or_default();
        let mut outcome = BakedPatchOutcome::default();

        for (concrete_path, slot_names) in pages {
            let Some(page) = self.store.read_page(&concrete_path)? else {
                continue;
            };

            for slot_name in slot_names {
                let Some(slot) = page.slots.iter().find(|slot| slot.name == slot_name) else {
                    self.mark_stale(
                        &mut outcome,
                        &concrete_path,
                        format!("slot `{slot_name}` missing from baked metadata"),
                    )?;
                    continue;
                };
                let Some(recompute) = self.recompute_fns.get(&slot.name) else {
                    self.mark_stale(
                        &mut outcome,
                        &concrete_path,
                        format!("slot `{}` has no registered recompute function", slot.name),
                    )?;
                    continue;
                };

                let replacement = match recompute(&key, &concrete_path)
                    .and_then(|value| value.render_for_slot(slot))
                {
                    Ok(replacement) => replacement,
                    Err(err) => {
                        self.mark_stale(&mut outcome, &concrete_path, err.to_string())?;
                        continue;
                    }
                };

                match self.store.patch_slot(&concrete_path, slot, &replacement) {
                    Ok(()) => {
                        outcome.patched_pages.push(concrete_path.clone());
                        outcome.patched_slots.push(BakedPatchedSlot {
                            concrete_path: concrete_path.clone(),
                            slot: slot.name.clone(),
                        });
                    }
                    Err(err) => {
                        self.mark_stale(&mut outcome, &concrete_path, err.to_string())?;
                    }
                }
            }
        }

        outcome.normalize();
        Ok(outcome)
    }

    fn mark_stale(
        &self,
        outcome: &mut BakedPatchOutcome,
        concrete_path: &str,
        reason: impl Into<String>,
    ) -> io::Result<()> {
        self.store.mark_stale(concrete_path, reason)?;
        outcome.stale_pages.push(concrete_path.to_string());
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BakedPatchOutcome {
    pub patched_pages: Vec<String>,
    pub stale_pages: Vec<String>,
    pub patched_slots: Vec<BakedPatchedSlot>,
}

impl BakedPatchOutcome {
    fn normalize(&mut self) {
        self.patched_pages.sort();
        self.patched_pages.dedup();
        self.stale_pages.sort();
        self.stale_pages.dedup();
        self.patched_slots.sort();
        self.patched_slots.dedup();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BakedPatchedSlot {
    pub concrete_path: String,
    pub slot: String,
}

pub fn text_slot_content(name: &str, value: &str) -> String {
    format!(
        "<span data-pilcrow-slot=\"{}\">{}</span>",
        escape_attr(name),
        escape_html(value)
    )
}

pub fn trusted_html_slot_content(html: TrustedHtml) -> String {
    html.into_string()
}

pub fn replace_slot_content(
    html: &str,
    slot: &str,
    kind: &BakedSlotKind,
    replacement: &str,
) -> io::Result<String> {
    validate_replacement(replacement)?;
    let boundary = find_slot_boundary(html, slot, kind)?;
    let mut patched = String::with_capacity(html.len() + replacement.len());
    patched.push_str(&html[..boundary.content_start]);
    patched.push_str(replacement);
    patched.push_str(&html[boundary.content_end..]);
    Ok(patched)
}

fn validate_replacement(replacement: &str) -> io::Result<()> {
    if replacement.contains("<!--pilcrow-slot:start")
        || replacement.contains("<!--pilcrow-slot:end")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "slot replacement must not contain Pilcrow slot markers",
        ));
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct SlotBoundary {
    content_start: usize,
    content_end: usize,
}

fn find_slot_boundary(html: &str, slot: &str, kind: &BakedSlotKind) -> io::Result<SlotBoundary> {
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

    Ok(SlotBoundary {
        content_start: start_pos + start.len(),
        content_end: end_pos,
    })
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

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_attr(value: &str) -> String {
    escape_html(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baked_pages::BakedPage;
    use std::fs;

    const DEP: &str = "entity:1";

    fn slot_marker(slot: &str, kind: &BakedSlotKind, content: &str) -> String {
        format!(
            "<!--pilcrow-slot:start {slot} kind={}-->{content}<!--pilcrow-slot:end {slot}-->",
            kind.marker_kind()
        )
    }

    fn page(store: &BakedPageStore, concrete_path: &str, slots: Vec<BakedSlot>) -> BakedPage {
        BakedPage::full_page(
            "/example",
            concrete_path,
            store.html_path(concrete_path).to_string_lossy().to_string(),
            store
                .metadata_path(concrete_path)
                .to_string_lossy()
                .to_string(),
            slots,
            1,
            "test-renderer",
        )
    }

    fn fragment_page(
        store: &BakedPageStore,
        concrete_path: &str,
        slots: Vec<BakedSlot>,
    ) -> BakedPage {
        BakedPage::fragment_composed(
            "/example",
            concrete_path,
            "app",
            store.body_path(concrete_path).to_string_lossy().to_string(),
            store
                .metadata_path(concrete_path)
                .to_string_lossy()
                .to_string(),
            slots,
            1,
            "test-renderer",
        )
    }

    fn text_slot(name: &str) -> BakedSlot {
        BakedSlot::text(name, vec![DependencyKey::new(DEP)])
    }

    fn html_slot(name: &str) -> BakedSlot {
        BakedSlot::trusted_html(name, vec![DependencyKey::new(DEP)])
    }

    #[test]
    fn successful_text_slot_patch() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let slot = text_slot("status");
        store
            .write_artifact(
                &page(&store, "/docs/intro", vec![slot.clone()]),
                &slot_marker("status", &BakedSlotKind::Text, "old"),
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
                "<span data-pilcrow-slot=\"status\">new</span>"
            )
        );
    }

    #[test]
    fn text_slot_patch_escapes_html() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        store
            .write_artifact(
                &page(&store, "/docs/intro", vec![text_slot("status")]),
                &slot_marker("status", &BakedSlotKind::Text, "old"),
            )
            .unwrap();
        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_slot_recompute("status", |_key, _page| {
            Ok(SlotValue::text("<script>alert('x')</script> & done"))
        });

        registry.patch_dependency(DependencyKey::new(DEP)).unwrap();

        let html = fs::read_to_string(store.html_path("/docs/intro")).unwrap();
        assert!(html.contains("&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt; &amp; done"));
        assert!(!html.contains("<script>alert"));
    }

    #[test]
    fn trusted_html_patch_requires_explicit_wrapper() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        store
            .write_artifact(
                &page(&store, "/docs/intro", vec![html_slot("status")]),
                &slot_marker("status", &BakedSlotKind::TrustedHtml, "<em>old</em>"),
            )
            .unwrap();
        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_slot_recompute("status", |_key, _page| {
            Ok(SlotValue::trusted_html(TrustedHtml::from_sanitized(
                "<strong>new</strong>",
            )))
        });

        registry.patch_dependency(DependencyKey::new(DEP)).unwrap();

        let html = fs::read_to_string(store.html_path("/docs/intro")).unwrap();
        assert!(html.contains("<strong>new</strong>"));
    }

    #[test]
    fn missing_marker_marks_page_stale() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        store
            .write_artifact(
                &page(&store, "/docs/intro", vec![text_slot("status")]),
                "<main>no marker</main>",
            )
            .unwrap();
        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_slot_recompute("status", |_key, _page| Ok(SlotValue::text("new")));

        let outcome = registry.patch_dependency(DependencyKey::new(DEP)).unwrap();

        assert_eq!(outcome.stale_pages, vec!["/docs/intro".to_string()]);
        assert!(
            store
                .read_page("/docs/intro")
                .unwrap()
                .unwrap()
                .stale_state
                .stale
        );
    }

    #[test]
    fn duplicate_marker_marks_page_stale() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let marker = slot_marker("status", &BakedSlotKind::Text, "one");
        store
            .write_artifact(
                &page(&store, "/docs/intro", vec![text_slot("status")]),
                &format!("{marker}{marker}"),
            )
            .unwrap();
        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_slot_recompute("status", |_key, _page| Ok(SlotValue::text("new")));

        let outcome = registry.patch_dependency(DependencyKey::new(DEP)).unwrap();

        assert_eq!(outcome.stale_pages, vec!["/docs/intro".to_string()]);
        assert!(store
            .read_page("/docs/intro")
            .unwrap()
            .unwrap()
            .stale_state
            .reason
            .unwrap()
            .contains("duplicate marker"));
    }

    #[test]
    fn malformed_marker_order_marks_page_stale() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        store
            .write_artifact(
                &page(&store, "/docs/intro", vec![text_slot("status")]),
                "<!--pilcrow-slot:end status-->old<!--pilcrow-slot:start status kind=text-->",
            )
            .unwrap();
        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_slot_recompute("status", |_key, _page| Ok(SlotValue::text("new")));

        let outcome = registry.patch_dependency(DependencyKey::new(DEP)).unwrap();

        assert_eq!(outcome.stale_pages, vec!["/docs/intro".to_string()]);
        assert!(store
            .read_page("/docs/intro")
            .unwrap()
            .unwrap()
            .stale_state
            .reason
            .unwrap()
            .contains("start marker appears after end marker"));
    }

    #[test]
    fn dependency_key_patches_multiple_pages() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        for path in ["/docs/intro", "/docs/summary"] {
            store
                .write_artifact(
                    &page(&store, path, vec![text_slot("status")]),
                    &slot_marker("status", &BakedSlotKind::Text, "old"),
                )
                .unwrap();
        }
        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_slot_recompute("status", |_key, page| {
            Ok(SlotValue::text(format!("new {page}")))
        });

        let outcome = registry.patch_dependency(DependencyKey::new(DEP)).unwrap();

        assert_eq!(
            outcome.patched_pages,
            vec!["/docs/intro".to_string(), "/docs/summary".to_string()]
        );
        assert!(fs::read_to_string(store.html_path("/docs/intro"))
            .unwrap()
            .contains("new /docs/intro"));
        assert!(fs::read_to_string(store.html_path("/docs/summary"))
            .unwrap()
            .contains("new /docs/summary"));
    }

    #[test]
    fn dependency_key_patches_multiple_declared_slots() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let slots = vec![text_slot("title"), text_slot("status")];
        let html = format!(
            "{}{}",
            slot_marker("title", &BakedSlotKind::Text, "old title"),
            slot_marker("status", &BakedSlotKind::Text, "old status")
        );
        store
            .write_artifact(&page(&store, "/docs/intro", slots), &html)
            .unwrap();
        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_slot_recompute("title", |_key, _page| Ok(SlotValue::text("new title")));
        registry.register_slot_recompute("status", |_key, _page| Ok(SlotValue::text("new status")));

        let outcome = registry.patch_dependency(DependencyKey::new(DEP)).unwrap();

        assert_eq!(outcome.patched_pages, vec!["/docs/intro".to_string()]);
        assert_eq!(outcome.patched_slots.len(), 2);
        let html = fs::read_to_string(store.html_path("/docs/intro")).unwrap();
        assert!(html.contains("new title"));
        assert!(html.contains("new status"));
    }

    #[test]
    fn recompute_failure_marks_affected_page_stale() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        store
            .write_artifact(
                &page(&store, "/docs/intro", vec![text_slot("status")]),
                &slot_marker("status", &BakedSlotKind::Text, "old"),
            )
            .unwrap();
        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_slot_recompute("status", |_key, _page| {
            Err(io::Error::new(io::ErrorKind::Other, "source unavailable"))
        });

        let outcome = registry.patch_dependency(DependencyKey::new(DEP)).unwrap();

        assert_eq!(outcome.stale_pages, vec!["/docs/intro".to_string()]);
        assert_eq!(
            store
                .read_page("/docs/intro")
                .unwrap()
                .unwrap()
                .stale_state
                .reason
                .as_deref(),
            Some("source unavailable")
        );
    }

    #[test]
    fn fragment_composed_patch_targets_body_not_layout() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        fs::create_dir_all(store.layout_path("app").parent().unwrap()).unwrap();
        fs::write(
            store.layout_path("app"),
            slot_marker("status", &BakedSlotKind::Text, "layout old"),
        )
        .unwrap();
        store
            .write_artifact(
                &fragment_page(&store, "/docs/intro", vec![text_slot("status")]),
                &slot_marker("status", &BakedSlotKind::Text, "body old"),
            )
            .unwrap();
        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_slot_recompute("status", |_key, _page| Ok(SlotValue::text("body new")));

        registry.patch_dependency(DependencyKey::new(DEP)).unwrap();

        assert!(fs::read_to_string(store.body_path("/docs/intro"))
            .unwrap()
            .contains("body new"));
        assert!(fs::read_to_string(store.layout_path("app"))
            .unwrap()
            .contains("layout old"));
    }
}
