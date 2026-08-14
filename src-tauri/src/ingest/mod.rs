//! The ingestion pipeline.
//!
//! ```text
//! file → content hash → Xberg extraction (native text or local OCR)
//!      → pii-vault scan → Card + extracted-text cache
//!      → structure-aware passages → Tantivy → Ready
//! ```
//!
//! Deterministic, no language model involved. Files are processed through a
//! bounded queue whose concurrency follows available memory, and each file
//! becomes searchable on its own the moment it finishes.

pub mod card;
pub mod extract;
pub mod passages;

use anyhow::Result;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::core::{read_lock, write_lock, Ctx};
use crate::ids;
use crate::types::{DocStatus, DocumentMeta, SourceType};

#[derive(Debug, Clone)]
pub struct ProcessJob {
    pub shelf_id: String,
    pub source_id: String,
    pub source_type: SourceType,
    pub source_label: String,
    pub abs_path: PathBuf,
    pub rel_path: String,
    pub force: bool,
}

#[derive(Debug, Clone)]
pub enum Job {
    Process(ProcessJob),
    RemoveDocument { shelf_id: String, doc_id: String },
}

#[derive(Clone)]
pub struct Ingestor {
    ctx: Arc<Ctx>,
    tx: mpsc::Sender<Job>,
}

fn persist_docs(ctx: &Ctx, library: &crate::shelf::Library, shelf_id: &str) {
    if let Err(error) = library.save_documents(&ctx.paths, shelf_id) {
        log::error!("saving document registry for {shelf_id}: {error:#}");
    }
}

impl Ingestor {
    /// Start the worker pool. Concurrency follows available memory so
    /// reading files never crowds out the rest of the machine.
    pub fn start(ctx: Arc<Ctx>) -> Self {
        let (tx, rx) = mpsc::channel::<Job>(2048);
        let rx = Arc::new(tokio::sync::Mutex::new(rx));
        let workers = Ctx::ingest_concurrency();
        log::info!("ingestion workers: {workers}");
        for _ in 0..workers {
            let ctx = ctx.clone();
            let rx = rx.clone();
            tokio::spawn(async move {
                loop {
                    let job = { rx.lock().await.recv().await };
                    let Some(job) = job else { break };
                    match job {
                        Job::Process(job) => {
                            if let Err(error) = process_file(&ctx, &job).await {
                                log::error!(
                                    "pipeline error for {}: {error:#}",
                                    job.abs_path.display()
                                );
                            }
                        }
                        Job::RemoveDocument { shelf_id, doc_id } => {
                            remove_document(&ctx, &shelf_id, &doc_id);
                        }
                    }
                }
            });
        }
        Self { ctx, tx }
    }

    pub async fn enqueue(&self, job: Job) {
        if self.tx.send(job).await.is_err() {
            log::error!("ingest queue closed");
        }
    }

    /// Queue every supported file of a shelf source (managed folder or one
    /// linked folder), and drop derived data of files that disappeared.
    pub async fn sync_source(
        &self,
        shelf_id: &str,
        source_id: &str,
        source_type: SourceType,
        source_label: &str,
        root: &Path,
        force: bool,
    ) {
        let files = {
            let root = root.to_path_buf();
            tokio::task::spawn_blocking(move || crate::shelf::scan_folder(&root))
                .await
                .unwrap_or_default()
        };

        // Remove docs whose file no longer exists.
        let stale: Vec<String> = {
            let library = read_lock(&self.ctx.library);
            library
                .documents(shelf_id)
                .into_iter()
                .filter(|d| d.source_id == source_id)
                .filter(|d| !Path::new(&d.path).exists())
                .map(|d| d.id)
                .collect()
        };
        for doc_id in stale {
            self.enqueue(Job::RemoveDocument {
                shelf_id: shelf_id.to_string(),
                doc_id,
            })
            .await;
        }

        for file in files {
            let rel = file
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| file.to_string_lossy().to_string());
            self.enqueue(Job::Process(ProcessJob {
                shelf_id: shelf_id.to_string(),
                source_id: source_id.to_string(),
                source_type,
                source_label: source_label.to_string(),
                abs_path: file,
                rel_path: rel,
                force,
            }))
            .await;
        }
    }

    /// Sync every source of every shelf (startup / manual refresh).
    pub async fn sync_all(&self, force: bool) {
        let shelves: Vec<crate::shelf::Shelf> = {
            let library = read_lock(&self.ctx.library);
            library.shelves().to_vec()
        };
        for shelf in shelves {
            self.sync_source(
                &shelf.id,
                crate::shelf::Shelf::IMPORTED_SOURCE,
                SourceType::Imported,
                "Imported",
                &shelf.managed_path,
                force,
            )
            .await;
            for linked in &shelf.linked_folders {
                let label = linked
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| linked.path.to_string_lossy().to_string());
                self.sync_source(
                    &shelf.id,
                    &ids::source_id(&linked.path.to_string_lossy()),
                    SourceType::Linked,
                    &label,
                    &linked.path,
                    force,
                )
                .await;
            }
        }
    }
}

fn emit_file_event(ctx: &Ctx, meta: &DocumentMeta) {
    ctx.events.emit(
        "rebost://ingest",
        json!({
            "shelfId": meta.shelf_id,
            "documentId": meta.id,
            "fileName": meta.file_name,
            "status": meta.status,
            "error": meta.error,
            "piiTotal": meta.pii_total,
            "passages": meta.passage_count,
        }),
    );
}

fn emit_shelf_stats(ctx: &Ctx, shelf_id: &str) {
    let stats = read_lock(&ctx.library).stats(shelf_id);
    ctx.events.emit(
        "rebost://shelf-stats",
        json!({ "shelfId": shelf_id, "stats": stats }),
    );
}

/// Turn an internal failure into product language (action-oriented, no
/// stack traces).
fn friendly_error(error: &anyhow::Error) -> String {
    if let Some(xberg_error) = error.downcast_ref::<xberg::XbergError>() {
        return match xberg_error {
            xberg::XbergError::UnsupportedFormat(_) => {
                "This file type isn't supported.".to_string()
            }
            xberg::XbergError::Ocr { .. } => {
                "Couldn't read this scan. Try again after checking the file opens normally."
                    .to_string()
            }
            xberg::XbergError::Timeout { .. } => "Reading took too long. Try again.".to_string(),
            xberg::XbergError::Io(_) => "Couldn't open this file. Try again.".to_string(),
            _ => "Couldn't read this file. Try again.".to_string(),
        };
    }
    let text = error.to_string();
    if text.contains("couldn't read any text") {
        "Rebost couldn't read any text in this file, even as a scan.".to_string()
    } else {
        "Couldn't read this file. Try again.".to_string()
    }
}

pub async fn process_file(ctx: &Arc<Ctx>, job: &ProcessJob) -> Result<()> {
    if std::fs::symlink_metadata(&job.abs_path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        log::warn!("skipping symlink {}", job.abs_path.display());
        return Ok(());
    }
    if !job.abs_path.exists() {
        let doc = {
            let library = read_lock(&ctx.library);
            library.find_document_by_path(&job.shelf_id, &job.abs_path)
        };
        if let Some(doc) = doc {
            remove_document(ctx, &job.shelf_id, &doc.id);
        }
        return Ok(());
    }

    let doc_id = ids::document_id(&job.shelf_id, &job.source_id, &job.rel_path);
    let hash = {
        let path = job.abs_path.clone();
        tokio::task::spawn_blocking(move || ids::content_hash_file(&path)).await??
    };

    // Unchanged content that is already Ready → nothing to do.
    {
        let library = read_lock(&ctx.library);
        if !job.force {
            if let Some(existing) = library.document(&job.shelf_id, &doc_id) {
                if existing.hash == hash && existing.status == DocStatus::Ready {
                    return Ok(());
                }
            }
        }
    }

    let file_name = job
        .abs_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| job.rel_path.clone());
    let format = job
        .abs_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let size_bytes = std::fs::metadata(&job.abs_path)
        .map(|m| m.len())
        .unwrap_or(0);
    let now = chrono::Utc::now().to_rfc3339();

    let mut meta = DocumentMeta {
        id: doc_id.clone(),
        shelf_id: job.shelf_id.clone(),
        source_id: job.source_id.clone(),
        source_type: job.source_type,
        path: job.abs_path.to_string_lossy().to_string(),
        rel_path: job.rel_path.clone(),
        file_name,
        format,
        size_bytes,
        hash: hash.clone(),
        status: DocStatus::Reading,
        error: None,
        passage_count: 0,
        pages: None,
        pii_total: 0,
        pii_categories: Default::default(),
        ocr: false,
        updated_at: now,
        source_label: job.source_label.clone(),
    };

    {
        let mut library = write_lock(&ctx.library);
        library.upsert_document(meta.clone());
        persist_docs(ctx, &library, &job.shelf_id);
    }
    emit_file_event(ctx, &meta);
    emit_shelf_stats(ctx, &job.shelf_id);

    // ── Extraction (native text or local OCR) ──────────────────────────
    let extraction = match extract::extract_file(&job.abs_path, &ctx.extractor).await {
        Ok(extraction) => extraction,
        Err(error) => {
            meta.status = DocStatus::Error;
            meta.error = Some(friendly_error(&error));
            meta.updated_at = chrono::Utc::now().to_rfc3339();
            {
                let mut library = write_lock(&ctx.library);
                library.upsert_document(meta.clone());
                persist_docs(ctx, &library, &job.shelf_id);
            }
            emit_file_event(ctx, &meta);
            emit_shelf_stats(ctx, &job.shelf_id);
            log::warn!(
                "extraction failed for {}: {error:#}",
                job.abs_path.display()
            );
            return Ok(());
        }
    };

    // ── Privacy Lens scan (locally; counts only) ───────────────────────
    let privacy = {
        let content = extraction.content.clone();
        let pii = &ctx.pii;
        // pii scan is CPU-bound regex work — keep it off the async threads.
        tokio::task::block_in_place(|| pii.summarize(&content))
    };

    // ── Card + extracted-text cache ────────────────────────────────────
    let card = card::build_card(card::CardInputs {
        doc_id: &doc_id,
        source_type: job.source_type,
        path: &meta.path,
        hash: &hash,
        title: &extraction.title,
        format: &meta.format,
        language: extraction.language_tag.as_deref(),
        summary: &extraction.summary,
        keywords: &extraction.keywords,
        outline: &extraction.outline,
        ocr_used: extraction.ocr_used,
        privacy: &privacy,
    });
    card::write_card(&ctx.paths.card_path(&job.shelf_id, &doc_id), &card)?;
    let extracted_path = ctx.paths.extracted_path(&job.shelf_id, &doc_id);
    if let Some(parent) = extracted_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&extracted_path, &extraction.content)?;

    // ── Passages → Tantivy ──────────────────────────────────────────────
    let passages = passages::build_passages(&extraction.blocks);
    let passage_count = passages.len() as u32;
    {
        let ctx2 = ctx.clone();
        let meta2 = meta.clone();
        let summary = extraction.summary.clone();
        let keywords = extraction.keywords.clone();
        let title = extraction.title.clone();
        let language = extraction.language;
        let quality = if extraction.ocr_used { "ocr" } else { "full" };
        tokio::task::spawn_blocking(move || {
            ctx2.search.index_document(
                &meta2, &summary, &keywords, &passages, language, quality, &title,
            )
        })
        .await??;
    }

    // ── Ready ───────────────────────────────────────────────────────────
    meta.status = DocStatus::Ready;
    meta.error = None;
    meta.passage_count = passage_count;
    meta.pages = extraction.page_count;
    meta.pii_total = privacy.total;
    meta.pii_categories = privacy.categories.clone();
    meta.ocr = extraction.ocr_used;
    meta.updated_at = chrono::Utc::now().to_rfc3339();
    {
        let mut library = write_lock(&ctx.library);
        library.upsert_document(meta.clone());
        persist_docs(ctx, &library, &job.shelf_id);
    }
    emit_file_event(ctx, &meta);
    emit_shelf_stats(ctx, &job.shelf_id);
    Ok(())
}

/// Remove one document's derived data (card, extracted text, index records,
/// registry entry). Never touches the original file.
pub fn remove_document(ctx: &Arc<Ctx>, shelf_id: &str, doc_id: &str) {
    {
        let mut library = write_lock(&ctx.library);
        library.remove_document(shelf_id, doc_id);
        persist_docs(ctx, &library, shelf_id);
    }
    std::fs::remove_file(ctx.paths.card_path(shelf_id, doc_id)).ok();
    std::fs::remove_file(ctx.paths.extracted_path(shelf_id, doc_id)).ok();
    if let Err(error) = ctx.search.remove_document(doc_id) {
        log::error!("removing {doc_id} from index: {error:#}");
    }
    ctx.events.emit(
        "rebost://ingest",
        json!({
            "shelfId": shelf_id,
            "documentId": doc_id,
            "status": "removed",
        }),
    );
    emit_shelf_stats(ctx, shelf_id);
}

/// Remove all derived data for a list of documents (source unlink / shelf
/// removal).
pub fn remove_documents(ctx: &Arc<Ctx>, shelf_id: &str, doc_ids: &[String]) {
    for doc_id in doc_ids {
        std::fs::remove_file(ctx.paths.card_path(shelf_id, doc_id)).ok();
        std::fs::remove_file(ctx.paths.extracted_path(shelf_id, doc_id)).ok();
    }
    if let Err(error) = ctx.search.remove_documents(doc_ids) {
        log::error!("removing documents from index: {error:#}");
    }
    emit_shelf_stats(ctx, shelf_id);
}
