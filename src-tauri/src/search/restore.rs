//! Recover derived search entries after an index format or tokenizer change.

use anyhow::{Context, Result};
use std::io::BufRead;

use crate::chat::conversations::{Conversations, StoredMessage};
use crate::core::Ctx;
use crate::shelf::Library;
use crate::types::DocStatus;

pub(super) const PENDING_FILE: &str = "rebost-rebuild-pending";

/// Restore before workers start, including cached files on disconnected drives.
/// A persistent marker retries incomplete restoration after the next launch.
pub(crate) fn restore_if_pending(ctx: &Ctx) -> Result<()> {
    let pending = ctx.paths.search_dir().join(PENDING_FILE);
    if !pending.try_exists()? {
        return Ok(());
    }
    let documents = {
        let library = Library::load_for_search_restore(&ctx.paths)?;
        library
            .shelves()
            .iter()
            .flat_map(|shelf| library.documents(&shelf.id))
            .filter(|meta| {
                meta.status == DocStatus::Ready
                    && meta.passage_count > 0
                    && library.source_is_live(&meta.shelf_id, &meta.source_id)
            })
            .collect::<Vec<_>>()
    };
    let threads = Conversations::list_for_search_restore(&ctx.paths)?;
    let mut failed = false;
    for meta in documents {
        if ctx.search.has_document(&meta.id) {
            continue;
        }
        if let Err(error) = crate::ingest::reindex_from_cache_deferred(ctx, &meta) {
            log::warn!("restore cached document {}: {error:#}", meta.id);
            failed = true;
        }
    }
    for thread in threads {
        let path = ctx.paths.thread_path(&thread.id);
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound && thread.message_count == 0 =>
            {
                continue;
            }
            Err(error) => {
                log::warn!("restore conversation {}: {error}", thread.id);
                failed = true;
                continue;
            }
        };
        for line in std::io::BufReader::new(file).lines() {
            let line = line.with_context(|| format!("read conversation {}", thread.id))?;
            // Match the conversation reader: tolerate a crash-truncated JSONL record.
            let Ok(message) = serde_json::from_str::<StoredMessage>(&line) else {
                continue;
            };
            if message.status == "error" || message.text.is_empty() {
                continue;
            }
            let date = chrono::DateTime::parse_from_rfc3339(&message.ts)
                .or_else(|_| chrono::DateTime::parse_from_rfc3339(&thread.created_at))
                .map(|date| date.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            ctx.search.index_message_deferred(
                &thread.id,
                &message.id,
                &message.role,
                &message.text,
                crate::chat::guess_message_lang(&message.text),
                date,
            )?;
        }
    }
    ctx.search.commit()?;
    if failed {
        anyhow::bail!("some persisted search entries could not be restored");
    }
    std::fs::remove_file(pending).context("finish search index restoration")?;
    Ok(())
}
