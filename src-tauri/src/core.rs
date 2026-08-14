//! The core application context — everything the pipeline, chat and
//! commands share. Deliberately independent of Tauri so the whole core is
//! testable headlessly; the app layer plugs in a real event sink.

use crate::ingest::extract::ExtractorSettings;
use crate::paths::Paths;
use crate::pii::PiiScanner;
use crate::search::SearchIndex;
use crate::settings::Settings;
use crate::shelf::Library;
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Recover a poisoned `RwLock` instead of crashing the desktop app.
pub(crate) fn read_lock<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) fn write_lock<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) fn mutex_lock<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Event sink towards the UI. The Tauri layer forwards to the webview;
/// tests collect events in memory.
pub trait Events: Send + Sync + 'static {
    fn emit(&self, event: &str, payload: serde_json::Value);
}

pub struct NoopEvents;

impl Events for NoopEvents {
    fn emit(&self, _event: &str, _payload: serde_json::Value) {}
}

pub struct Ctx {
    pub paths: Paths,
    pub settings: RwLock<Settings>,
    pub pii: PiiScanner,
    pub search: SearchIndex,
    pub library: RwLock<Library>,
    pub events: Arc<dyn Events>,
    pub extractor: ExtractorSettings,
}

impl Ctx {
    pub fn new(
        paths: Paths,
        events: Arc<dyn Events>,
        extractor: ExtractorSettings,
    ) -> anyhow::Result<Arc<Self>> {
        paths.ensure()?;
        let settings = Settings::load(&paths.settings_path());
        let search = SearchIndex::open(&paths.search_dir())?;
        let library = Library::load(&paths)?;
        Ok(Arc::new(Self {
            paths,
            settings: RwLock::new(settings),
            pii: PiiScanner::new(),
            search,
            library: RwLock::new(library),
            events,
            extractor,
        }))
    }

    pub fn save_settings(&self) {
        let settings = read_lock(&self.settings).clone();
        if let Err(error) = settings.save(&self.paths.settings_path()) {
            log::error!("saving settings: {error:#}");
        }
    }

    /// Local-context budget in characters — measured by the installation
    /// benchmark, defaulting conservatively before it runs.
    pub fn context_budget(&self) -> usize {
        read_lock(&self.settings)
            .context_budget_chars
            .unwrap_or(crate::search::gate::tuning::DEFAULT_BUDGET_CHARS)
    }

    /// Ingestion concurrency from available memory: one worker per ~6 GB
    /// free, between 1 and 4 — index work must never starve the machine.
    pub fn ingest_concurrency() -> usize {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        let available = sys.available_memory();
        ((available / (6 * 1024 * 1024 * 1024)) as usize).clamp(1, 4)
    }
}
