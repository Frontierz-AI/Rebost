//! Folder synchronization via `notify`, for managed Shelf folders and
//! linked sources.
//!
//! ```text
//! new file     → process
//! changed file → hash → reprocess when content changed
//! renamed file → old path drops derived data, new path processes
//! deleted file → remove derived document data and search records
//! ```
//!
//! Raw events are debounced and drained into the bounded ingestion queue.

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::core::Ctx;
use crate::ids;
use crate::ingest::{Ingestor, Job, ProcessJob};
use crate::types::SourceType;

const DEBOUNCE: Duration = Duration::from_millis(800);
const TICK: Duration = Duration::from_millis(400);
const MAX_PENDING: usize = 10_000;

pub struct WatcherHub {
    ctx: Arc<Ctx>,
    ingestor: Ingestor,
    watcher: Mutex<Option<RecommendedWatcher>>,
    tx: mpsc::UnboundedSender<PathBuf>,
}

struct SourceMatch {
    shelf_id: String,
    source_id: String,
    source_type: SourceType,
    source_label: String,
    root: PathBuf,
}

impl WatcherHub {
    pub fn start(ctx: Arc<Ctx>, ingestor: Ingestor) -> Arc<Self> {
        let (tx, mut rx) = mpsc::unbounded_channel::<PathBuf>();
        let hub = Arc::new(Self {
            ctx,
            ingestor,
            watcher: Mutex::new(None),
            tx,
        });
        hub.rebuild();

        // Debounce loop: collect changed paths, flush the quiet ones.
        let hub2 = hub.clone();
        tokio::spawn(async move {
            let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
            let mut ticker = tokio::time::interval(TICK);
            loop {
                tokio::select! {
                    maybe_path = rx.recv() => {
                        match maybe_path {
                            Some(path) => {
                                if pending.len() < MAX_PENDING {
                                    pending.insert(path, Instant::now());
                                }
                            }
                            None => break,
                        }
                    }
                    _ = ticker.tick() => {
                        let now = Instant::now();
                        let ready: Vec<PathBuf> = pending
                            .iter()
                            .filter(|(_, t)| now.duration_since(**t) >= DEBOUNCE)
                            .map(|(p, _)| p.clone())
                            .collect();
                        for path in ready {
                            pending.remove(&path);
                            hub2.handle_path(&path).await;
                        }
                    }
                }
            }
        });
        hub
    }

    /// (Re)watch every shelf root — call after shelves or links change.
    pub fn rebuild(&self) {
        let roots: Vec<PathBuf> = {
            let library = crate::core::read_lock(&self.ctx.library);
            library
                .shelves()
                .iter()
                .flat_map(|s| {
                    std::iter::once(s.managed_path.clone())
                        .chain(s.linked_folders.iter().map(|l| l.path.clone()))
                })
                .collect()
        };
        let tx = self.tx.clone();
        let mut watcher = match notify::recommended_watcher(move |result: notify::Result<Event>| {
            let Ok(event) = result else { return };
            if !matches!(
                event.kind,
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
            ) {
                return;
            }
            for path in event.paths {
                if path
                    .file_name()
                    .map(|n| n.to_string_lossy().starts_with('.'))
                    .unwrap_or(true)
                {
                    continue;
                }
                tx.send(path).ok();
            }
        }) {
            Ok(watcher) => watcher,
            Err(error) => {
                log::error!("could not create filesystem watcher: {error:#}");
                return;
            }
        };
        for root in &roots {
            if let Err(error) = watcher.watch(root, RecursiveMode::Recursive) {
                log::warn!("cannot watch {}: {error:#}", root.display());
            }
        }
        *crate::core::mutex_lock(&self.watcher) = Some(watcher);
        log::info!("watching {} folder(s)", roots.len());
    }

    fn resolve(&self, path: &Path) -> Option<SourceMatch> {
        let library = crate::core::read_lock(&self.ctx.library);
        let mut best: Option<SourceMatch> = None;
        for shelf in library.shelves() {
            if path.starts_with(&shelf.managed_path) {
                let candidate = SourceMatch {
                    shelf_id: shelf.id.clone(),
                    source_id: crate::shelf::Shelf::IMPORTED_SOURCE.to_string(),
                    source_type: SourceType::Imported,
                    source_label: "Imported".to_string(),
                    root: shelf.managed_path.clone(),
                };
                if best
                    .as_ref()
                    .map(|b| candidate.root.components().count() > b.root.components().count())
                    .unwrap_or(true)
                {
                    best = Some(candidate);
                }
            }
            for linked in &shelf.linked_folders {
                if path.starts_with(&linked.path) {
                    let label = linked
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| linked.path.to_string_lossy().to_string());
                    let candidate = SourceMatch {
                        shelf_id: shelf.id.clone(),
                        source_id: ids::source_id(&linked.path.to_string_lossy()),
                        source_type: SourceType::Linked,
                        source_label: label,
                        root: linked.path.clone(),
                    };
                    if best
                        .as_ref()
                        .map(|b| candidate.root.components().count() > b.root.components().count())
                        .unwrap_or(true)
                    {
                        best = Some(candidate);
                    }
                }
            }
        }
        best
    }

    async fn handle_path(&self, path: &Path) {
        if std::fs::symlink_metadata(path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            return;
        }
        let Some(source) = self.resolve(path) else {
            return;
        };
        if path.is_dir() {
            // A folder appeared or changed — sync that source subtree.
            self.ingestor
                .sync_source(
                    &source.shelf_id,
                    &source.source_id,
                    source.source_type,
                    &source.source_label,
                    &source.root,
                    false,
                )
                .await;
            return;
        }
        if path.exists() {
            if !crate::ingest::extract::is_supported_file(path) {
                return;
            }
            let Ok(rel) = path.strip_prefix(&source.root) else {
                return;
            };
            if rel
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
            {
                return;
            }
            let rel = rel.to_string_lossy().to_string();
            self.ingestor
                .enqueue(Job::Process(ProcessJob {
                    shelf_id: source.shelf_id,
                    source_id: source.source_id,
                    source_type: source.source_type,
                    source_label: source.source_label,
                    abs_path: path.to_path_buf(),
                    rel_path: rel,
                    force: false,
                }))
                .await;
        } else {
            // Deleted or renamed away: drop derived data for this path, and
            // for anything that lived under it if it was a folder.
            let docs: Vec<(String, String)> = {
                let library = crate::core::read_lock(&self.ctx.library);
                let prefix = format!("{}{}", path.to_string_lossy(), std::path::MAIN_SEPARATOR);
                library
                    .documents(&source.shelf_id)
                    .into_iter()
                    .filter(|d| d.path == path.to_string_lossy() || d.path.starts_with(&prefix))
                    .map(|d| (d.shelf_id, d.id))
                    .collect()
            };
            for (shelf_id, doc_id) in docs {
                self.ingestor
                    .enqueue(Job::RemoveDocument { shelf_id, doc_id })
                    .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::NoopEvents;
    use crate::ingest::extract::ExtractorSettings;
    use crate::paths::Paths;
    use crate::types::DocStatus;

    impl WatcherHub {
        fn inject(&self, path: PathBuf) {
            self.tx.send(path).ok();
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn injected_create_is_ingested_after_debounce() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("appdata"));
        let ctx = Ctx::new(paths, Arc::new(NoopEvents), ExtractorSettings::default()).unwrap();
        let root = dir.path().join("Rebost");
        let shelf = {
            let mut library = crate::core::write_lock(&ctx.library);
            library.create_shelf(&ctx.paths, "Watch", &root).unwrap()
        };
        let file = shelf.managed_path.join("hello.md");
        std::fs::write(&file, "# Hello\n\nWatched file body.\n").unwrap();

        let ingestor = Ingestor::start(ctx.clone());
        let hub = WatcherHub::start(ctx.clone(), ingestor);
        hub.inject(file.clone());

        let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
        loop {
            let docs = crate::core::read_lock(&ctx.library).documents(&shelf.id);
            if docs
                .iter()
                .any(|d| d.file_name == "hello.md" && d.status != DocStatus::Reading)
            {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!(
                    "debounce/ingest did not pick up hello.md; docs={:?}",
                    docs.iter()
                        .map(|d| (&d.file_name, d.status))
                        .collect::<Vec<_>>()
                );
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    }
}
