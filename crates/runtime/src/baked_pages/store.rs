use super::{
    replace_slot_content, BakedArtifactMode, BakedPage, BakedSlot, DependencyKey, StaleState,
};
use std::{
    collections::BTreeMap,
    fs,
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub type ReverseIndex = BTreeMap<String, BTreeMap<String, Vec<String>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakedArtifactHit {
    pub page: BakedPage,
    pub html: String,
}

#[derive(Debug, Clone)]
pub struct BakedPageStore {
    root: PathBuf,
}

impl BakedPageStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write_artifact(&self, page: &BakedPage, html: &str) -> io::Result<BakedPage> {
        let mut page = self.normalize_page_for_write(page.clone());
        page.stale_state = StaleState::fresh();
        page.dependency_keys = collect_dependency_keys(&page.slots);

        self.write_atomic(Path::new(&page.body_path), html.as_bytes())?;
        self.write_page(&page)?;
        self.upsert_reverse_index_page(&page)?;

        Ok(page)
    }

    pub fn serve_if_fresh(&self, concrete_path: &str) -> io::Result<Option<BakedArtifactHit>> {
        let Some(page) = self.read_page(concrete_path)? else {
            return Ok(None);
        };
        if page.stale_state.stale {
            return Ok(None);
        }

        match fs::read_to_string(&page.body_path) {
            Ok(html) => {
                self.touch(concrete_path)?;
                let page = self.read_page(concrete_path)?.unwrap_or(page);
                Ok(Some(BakedArtifactHit { page, html }))
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub fn read_page(&self, concrete_path: &str) -> io::Result<Option<BakedPage>> {
        let raw = match fs::read_to_string(self.metadata_path(concrete_path)) {
            Ok(raw) => raw,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        serde_json::from_str(&raw)
            .map(|page| Some(self.normalize_page_for_read(page)))
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }

    pub fn mark_stale(&self, concrete_path: &str, reason: impl Into<String>) -> io::Result<()> {
        let mut page = self.read_page(concrete_path)?.unwrap_or_else(|| {
            let now = unix_timestamp();
            BakedPage::full_page(
                "",
                concrete_path,
                self.html_path(concrete_path).to_string_lossy().to_string(),
                self.metadata_path(concrete_path)
                    .to_string_lossy()
                    .to_string(),
                Vec::new(),
                now,
                "",
            )
        });
        page.stale_state = StaleState::stale(reason);
        self.write_page(&page)
    }

    pub fn patch_slot(
        &self,
        concrete_path: &str,
        slot: &BakedSlot,
        replacement: &str,
    ) -> io::Result<()> {
        let Some(mut page) = self.read_page(concrete_path)? else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("baked page metadata not found for `{concrete_path}`"),
            ));
        };
        let html = fs::read_to_string(&page.body_path)?;
        let patched = replace_slot_content(&html, &slot.name, &slot.kind, replacement)?;
        self.write_atomic(Path::new(&page.body_path), patched.as_bytes())?;
        page.stale_state = StaleState::fresh();
        self.write_page(&page)
    }

    pub fn ensure_reverse_index(&self) -> io::Result<ReverseIndex> {
        match self.reverse_index() {
            Ok(Some(index)) => Ok(index),
            Ok(None) => {
                let index = self.rebuild_reverse_index_from_metadata()?;
                self.write_reverse_index(&index)?;
                Ok(index)
            }
            Err(err) if err.kind() == io::ErrorKind::InvalidData => {
                let index = self.rebuild_reverse_index_from_metadata()?;
                self.write_reverse_index(&index)?;
                Ok(index)
            }
            Err(err) => Err(err),
        }
    }

    pub fn rebuild_reverse_index_from_metadata(&self) -> io::Result<ReverseIndex> {
        let mut index = ReverseIndex::new();
        for path in self.metadata_files()? {
            let raw = fs::read_to_string(path)?;
            let page: BakedPage = serde_json::from_str(&raw)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            let page = self.normalize_page_for_read(page);
            for slot in &page.slots {
                for dep in &slot.dependency_keys {
                    add_index_slot(&mut index, dep, &page.concrete_path, &slot.name);
                }
            }
        }

        Ok(index)
    }

    pub fn reverse_index(&self) -> io::Result<Option<ReverseIndex>> {
        match fs::read_to_string(self.reverse_index_path()) {
            Ok(raw) => serde_json::from_str(&raw)
                .map(Some)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub fn write_reverse_index(&self, index: &ReverseIndex) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(index).map_err(io::Error::other)?;
        self.write_atomic(&self.reverse_index_path(), &json)
    }

    pub fn html_path(&self, concrete_path: &str) -> PathBuf {
        self.root.join("pages").join(storage_name(concrete_path))
    }

    pub fn body_path(&self, concrete_path: &str) -> PathBuf {
        self.route_dir(concrete_path).join("body.html")
    }

    pub fn metadata_path(&self, concrete_path: &str) -> PathBuf {
        self.route_dir(concrete_path).join("metadata.json")
    }

    pub fn layout_path(&self, key: &str) -> PathBuf {
        self.root
            .join("layouts")
            .join(format!("{}.html", safe_key(key)))
    }

    pub fn fragment_path(&self, key: &str) -> PathBuf {
        self.root
            .join("fragments")
            .join(format!("{}.html", safe_key(key)))
    }

    pub fn reverse_index_path(&self) -> PathBuf {
        self.root.join("reverse-index.json")
    }

    fn write_page(&self, page: &BakedPage) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(page).map_err(io::Error::other)?;
        self.write_atomic(&self.metadata_path(&page.concrete_path), &json)
    }

    fn touch(&self, concrete_path: &str) -> io::Result<()> {
        if let Some(mut page) = self.read_page(concrete_path)? {
            page.last_accessed_at = unix_timestamp();
            self.write_page(&page)?;
        }
        Ok(())
    }

    fn upsert_reverse_index_page(&self, page: &BakedPage) -> io::Result<()> {
        let mut index = match self.reverse_index() {
            Ok(Some(index)) => index,
            Ok(None) => ReverseIndex::new(),
            Err(err) if err.kind() == io::ErrorKind::InvalidData => ReverseIndex::new(),
            Err(err) => return Err(err),
        };

        for pages in index.values_mut() {
            pages.remove(&page.concrete_path);
        }
        index.retain(|_, pages| !pages.is_empty());

        for slot in &page.slots {
            for dep in &slot.dependency_keys {
                add_index_slot(&mut index, dep, &page.concrete_path, &slot.name);
            }
        }

        self.write_reverse_index(&index)
    }

    fn metadata_files(&self) -> io::Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.collect_metadata_files(&self.root.join("pages"), &mut files)?;
        files.sort();
        files.dedup();
        Ok(files)
    }

    fn collect_metadata_files(&self, dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.collect_metadata_files(&path, files)?;
            } else if path.file_name().and_then(|name| name.to_str()) == Some("metadata.json") {
                files.push(path);
            }
        }
        Ok(())
    }

    fn normalize_page_for_write(&self, mut page: BakedPage) -> BakedPage {
        page.body_path = match page.artifact_mode {
            BakedArtifactMode::FullPage => self.html_path(&page.concrete_path),
            BakedArtifactMode::FragmentComposed => self.body_path(&page.concrete_path),
        }
        .to_string_lossy()
        .to_string();
        page.html_path = page.body_path.clone();
        page.metadata_path = self
            .metadata_path(&page.concrete_path)
            .to_string_lossy()
            .to_string();
        page
    }

    fn normalize_page_for_read(&self, mut page: BakedPage) -> BakedPage {
        if page.body_path.is_empty() {
            page.body_path = if page.html_path.is_empty() {
                match page.artifact_mode {
                    BakedArtifactMode::FullPage => self.html_path(&page.concrete_path),
                    BakedArtifactMode::FragmentComposed => self.body_path(&page.concrete_path),
                }
                .to_string_lossy()
                .to_string()
            } else {
                page.html_path.clone()
            };
        }
        if page.html_path.is_empty() {
            page.html_path = page.body_path.clone();
        }
        page.metadata_path = self
            .metadata_path(&page.concrete_path)
            .to_string_lossy()
            .to_string();
        page
    }

    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp = temp_path_for(path);
        {
            let mut file = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        fs::rename(&tmp, path)?;
        if let Some(parent) = path.parent() {
            if let Ok(dir) = OpenOptions::new().read(true).open(parent) {
                let _ = dir.sync_all();
            }
        }
        Ok(())
    }

    fn route_dir(&self, concrete_path: &str) -> PathBuf {
        let trimmed = concrete_path.trim_start_matches('/');
        if trimmed.is_empty() {
            self.root.join("pages").join("index")
        } else {
            trimmed
                .split('/')
                .filter(|segment| !segment.is_empty())
                .fold(self.root.join("pages"), |path, segment| {
                    path.join(safe_key(segment))
                })
        }
    }
}

fn add_index_slot(index: &mut ReverseIndex, dep: &DependencyKey, concrete_path: &str, slot: &str) {
    let slots = index
        .entry(dep.as_str().to_string())
        .or_default()
        .entry(concrete_path.to_string())
        .or_default();
    if !slots.iter().any(|existing| existing == slot) {
        slots.push(slot.to_string());
        slots.sort();
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

fn storage_name(concrete_path: &str) -> String {
    let trimmed = concrete_path.trim_matches('/');
    if trimmed.is_empty() {
        "index.html".to_string()
    } else {
        format!("{}.html", trimmed.replace('/', "__"))
    }
}

fn safe_key(key: &str) -> String {
    key.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '[' | ']') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn temp_path_for(path: &Path) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let pid = std::process::id();
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact");
    path.with_file_name(format!(".{filename}.{pid}.{nonce}.tmp"))
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
    use crate::baked_pages::BakedSlotKind;

    fn page(
        store: &BakedPageStore,
        concrete_path: &str,
        artifact_mode: BakedArtifactMode,
    ) -> BakedPage {
        let slots = vec![BakedSlot::text(
            "status",
            vec![DependencyKey::new(format!("page:{concrete_path}:status"))],
        )];
        match artifact_mode {
            BakedArtifactMode::FullPage => BakedPage::full_page(
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
            ),
            BakedArtifactMode::FragmentComposed => BakedPage::fragment_composed(
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
            ),
        }
    }

    #[test]
    fn writes_full_page_artifact_and_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let written = store
            .write_artifact(
                &page(&store, "/docs/intro", BakedArtifactMode::FullPage),
                "<html>intro</html>",
            )
            .unwrap();

        assert_eq!(
            fs::read_to_string(store.html_path("/docs/intro")).unwrap(),
            "<html>intro</html>"
        );
        assert_eq!(written.body_path, written.html_path);

        let metadata = store.read_page("/docs/intro").unwrap().unwrap();
        assert_eq!(metadata.artifact_mode, BakedArtifactMode::FullPage);
        assert_eq!(
            metadata.dependency_keys[0].as_str(),
            "page:/docs/intro:status"
        );
    }

    #[test]
    fn writes_fragment_composed_body_artifact_and_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let written = store
            .write_artifact(
                &page(&store, "/docs/intro", BakedArtifactMode::FragmentComposed),
                "<main>body</main>",
            )
            .unwrap();

        assert_eq!(
            fs::read_to_string(store.body_path("/docs/intro")).unwrap(),
            "<main>body</main>"
        );
        assert_eq!(written.layout_key.as_deref(), Some("app"));
        assert_eq!(written.body_path, written.html_path);

        let metadata = store.read_page("/docs/intro").unwrap().unwrap();
        assert_eq!(metadata.artifact_mode, BakedArtifactMode::FragmentComposed);
        assert_eq!(metadata.layout_key.as_deref(), Some("app"));
    }

    #[test]
    fn body_paths_are_route_shaped() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());

        assert_eq!(
            store.body_path("/tickets/123/summary"),
            temp.path()
                .join("pages")
                .join("tickets")
                .join("123")
                .join("summary")
                .join("body.html")
        );
        assert_eq!(
            store.metadata_path("/tickets/123/summary"),
            temp.path()
                .join("pages")
                .join("tickets")
                .join("123")
                .join("summary")
                .join("metadata.json")
        );
    }

    #[test]
    fn reverse_index_rebuilds_from_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        store
            .write_artifact(
                &page(&store, "/docs/intro", BakedArtifactMode::FullPage),
                "<html>intro</html>",
            )
            .unwrap();
        fs::remove_file(store.reverse_index_path()).unwrap();

        let index = store.ensure_reverse_index().unwrap();

        assert_eq!(
            index["page:/docs/intro:status"]["/docs/intro"],
            vec!["status".to_string()]
        );
        assert!(store.reverse_index_path().exists());
    }

    #[test]
    fn mark_stale_refuses_fresh_serving() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        store
            .write_artifact(
                &page(&store, "/docs/intro", BakedArtifactMode::FullPage),
                "<html>intro</html>",
            )
            .unwrap();

        assert!(store.serve_if_fresh("/docs/intro").unwrap().is_some());

        store.mark_stale("/docs/intro", "source changed").unwrap();

        assert!(store.serve_if_fresh("/docs/intro").unwrap().is_none());
        let metadata = store.read_page("/docs/intro").unwrap().unwrap();
        assert_eq!(
            metadata.stale_state.reason.as_deref(),
            Some("source changed")
        );
    }

    #[test]
    fn atomic_temp_file_is_not_served_output() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        store
            .write_artifact(
                &page(&store, "/docs/intro", BakedArtifactMode::FullPage),
                "<html>stable</html>",
            )
            .unwrap();

        let temp_artifact = store
            .html_path("/docs/intro")
            .with_file_name(".intro.html.partial.tmp");
        fs::write(temp_artifact, "<html>partial</html>").unwrap();

        let hit = store.serve_if_fresh("/docs/intro").unwrap().unwrap();

        assert_eq!(hit.html, "<html>stable</html>");
    }

    #[test]
    fn slot_kind_marker_values_match_storage_markers() {
        assert_eq!(BakedSlotKind::Text.marker_kind(), "text");
        assert_eq!(BakedSlotKind::TrustedHtml.marker_kind(), "html");
    }
}
