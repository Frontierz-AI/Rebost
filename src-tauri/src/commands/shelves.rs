//! Shelf, document, and file-manager commands.

use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use super::{friendly, require_id, CmdResult};
use crate::core::Ctx;
use crate::ingest::{Ingestor, Job, ProcessJob};
use crate::shelf::watcher::WatcherHub;
use crate::types::{Card, DocumentMeta, ShelfStats, SourceType};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedView {
    pub source_id: String,
    pub path: String,
    pub label: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfView {
    pub id: String,
    pub name: String,
    pub managed_path: String,
    pub linked_folders: Vec<LinkedView>,
    pub stats: ShelfStats,
}

fn shelf_view(shelf: &crate::shelf::Shelf, stats: ShelfStats) -> ShelfView {
    ShelfView {
        id: shelf.id.clone(),
        name: shelf.name.clone(),
        managed_path: shelf.managed_path.to_string_lossy().to_string(),
        linked_folders: shelf
            .linked_folders
            .iter()
            .map(|l| LinkedView {
                source_id: crate::ids::source_id(&l.path.to_string_lossy()),
                path: l.path.to_string_lossy().to_string(),
                label: l
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| l.path.to_string_lossy().to_string()),
            })
            .collect(),
        stats,
    }
}

#[tauri::command]
pub fn shelves_list(ctx: State<'_, Arc<Ctx>>) -> Vec<ShelfView> {
    let library = crate::core::read_lock(&ctx.library);
    library
        .shelves()
        .iter()
        .map(|s| shelf_view(s, library.stats(&s.id)))
        .collect()
}

#[tauri::command]
pub async fn shelf_create(
    ctx: State<'_, Arc<Ctx>>,
    watcher: State<'_, Arc<WatcherHub>>,
    name: String,
) -> CmdResult<ShelfView> {
    let root = crate::core::read_lock(&ctx.settings).shelf_root();
    let shelf = {
        let mut library = crate::core::write_lock(&ctx.library);
        library
            .create_shelf(&ctx.paths, &name, &root)
            .map_err(friendly)?
    };
    watcher.rebuild();
    ctx.events.emit("rebost://shelves", json!({}));
    Ok(shelf_view(
        &shelf,
        crate::core::read_lock(&ctx.library).stats(&shelf.id),
    ))
}

#[tauri::command]
pub async fn shelf_remove(
    ctx: State<'_, Arc<Ctx>>,
    watcher: State<'_, Arc<WatcherHub>>,
    shelf_id: String,
) -> CmdResult<()> {
    require_id(&shelf_id)?;
    let doc_ids = {
        let mut library = crate::core::write_lock(&ctx.library);
        library
            .remove_shelf(&ctx.paths, &shelf_id)
            .map_err(friendly)?
    };
    ctx.search.remove_shelf(&shelf_id).map_err(friendly)?;
    crate::ingest::remove_documents(&ctx, &shelf_id, &doc_ids);
    watcher.rebuild();
    ctx.events.emit("rebost://shelves", json!({}));
    Ok(())
}

/// "Add folder from this computer" — folder picker + link + scan.
#[tauri::command]
pub async fn shelf_add_linked(
    app: AppHandle,
    ctx: State<'_, Arc<Ctx>>,
    ingestor: State<'_, Ingestor>,
    watcher: State<'_, Arc<WatcherHub>>,
    shelf_id: String,
) -> CmdResult<Option<ShelfView>> {
    require_id(&shelf_id)?;
    let Some(folder) = app.dialog().file().blocking_pick_folder() else {
        return Ok(None);
    };
    let folder: PathBuf = folder.into_path().map_err(friendly)?;
    let shelf = {
        let mut library = crate::core::write_lock(&ctx.library);
        library
            .add_linked_folder(&shelf_id, &folder)
            .map_err(friendly)?
    };
    watcher.rebuild();
    let source_id = crate::ids::source_id(&folder.to_string_lossy());
    let label = folder
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    ingestor
        .sync_source(
            &shelf_id,
            &source_id,
            SourceType::Linked,
            &label,
            &folder,
            false,
        )
        .await;
    ctx.events.emit("rebost://shelves", json!({}));
    Ok(Some(shelf_view(
        &shelf,
        crate::core::read_lock(&ctx.library).stats(&shelf.id),
    )))
}

#[tauri::command]
pub async fn shelf_remove_source(
    ctx: State<'_, Arc<Ctx>>,
    watcher: State<'_, Arc<WatcherHub>>,
    shelf_id: String,
    source_id: String,
) -> CmdResult<()> {
    require_id(&shelf_id)?;
    require_id(&source_id)?;
    let doc_ids = {
        let mut library = crate::core::write_lock(&ctx.library);
        let ids = library
            .remove_linked_folder(&shelf_id, &source_id)
            .map_err(friendly)?;
        if let Err(error) = library.save_documents(&ctx.paths, &shelf_id) {
            log::error!("saving document registry for {shelf_id}: {error:#}");
        }
        ids
    };
    crate::ingest::remove_documents(&ctx, &shelf_id, &doc_ids);
    watcher.rebuild();
    ctx.events.emit("rebost://shelves", json!({}));
    Ok(())
}

/// Drag & drop onto a Shelf: copy into a fresh import folder, process.
#[tauri::command]
pub async fn shelf_import_paths(
    ctx: State<'_, Arc<Ctx>>,
    ingestor: State<'_, Ingestor>,
    shelf_id: String,
    paths: Vec<String>,
) -> CmdResult<u32> {
    require_id(&shelf_id)?;
    let shelf = crate::core::read_lock(&ctx.library)
        .shelf(&shelf_id)
        .cloned()
        .ok_or("Shelf not found")?;
    let dropped: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let (_, copied) = {
        let shelf = shelf.clone();
        tokio::task::spawn_blocking(move || crate::shelf::import_into_shelf(&shelf, &dropped))
            .await
            .map_err(friendly)?
            .map_err(friendly)?
    };
    let mut queued = 0u32;
    for file in &copied {
        if !crate::ingest::extract::is_supported_file(file) {
            continue;
        }
        let rel = file
            .strip_prefix(&shelf.managed_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file.to_string_lossy().to_string());
        ingestor
            .enqueue(Job::Process(ProcessJob {
                shelf_id: shelf_id.clone(),
                source_id: crate::shelf::Shelf::IMPORTED_SOURCE.to_string(),
                source_type: SourceType::Imported,
                source_label: "Imported".to_string(),
                abs_path: file.clone(),
                rel_path: rel,
                force: false,
            }))
            .await;
        queued += 1;
    }
    Ok(queued)
}

/// "Add files" button — file picker variant of import.
#[tauri::command]
pub async fn shelf_import_dialog(
    app: AppHandle,
    ctx: State<'_, Arc<Ctx>>,
    ingestor: State<'_, Ingestor>,
    shelf_id: String,
) -> CmdResult<u32> {
    require_id(&shelf_id)?;
    let Some(files) = app.dialog().file().blocking_pick_files() else {
        return Ok(0);
    };
    let paths: Vec<String> = files
        .into_iter()
        .filter_map(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    shelf_import_paths(ctx, ingestor, shelf_id, paths).await
}

#[tauri::command]
pub fn shelf_documents(ctx: State<'_, Arc<Ctx>>, shelf_id: String) -> CmdResult<Vec<DocumentMeta>> {
    require_id(&shelf_id)?;
    let mut docs = crate::core::read_lock(&ctx.library).documents(&shelf_id);
    docs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(docs)
}

#[tauri::command]
pub fn document_card(
    ctx: State<'_, Arc<Ctx>>,
    shelf_id: String,
    doc_id: String,
) -> CmdResult<Card> {
    require_id(&shelf_id)?;
    require_id(&doc_id)?;
    crate::ingest::card::read_card(&ctx.paths.card_path(&shelf_id, &doc_id)).map_err(friendly)
}

#[tauri::command]
pub fn document_text(
    ctx: State<'_, Arc<Ctx>>,
    shelf_id: String,
    doc_id: String,
) -> CmdResult<String> {
    require_id(&shelf_id)?;
    require_id(&doc_id)?;
    let text = std::fs::read_to_string(ctx.paths.extracted_path(&shelf_id, &doc_id))
        .map_err(|_| "No extracted text yet.".to_string())?;
    Ok(text.chars().take(400_000).collect())
}

#[tauri::command]
pub async fn document_reprocess(
    ctx: State<'_, Arc<Ctx>>,
    ingestor: State<'_, Ingestor>,
    shelf_id: String,
    doc_id: String,
) -> CmdResult<()> {
    require_id(&shelf_id)?;
    require_id(&doc_id)?;
    let doc = crate::core::read_lock(&ctx.library)
        .document(&shelf_id, &doc_id)
        .ok_or("File not found")?;
    ingestor
        .enqueue(Job::Process(ProcessJob {
            shelf_id: doc.shelf_id,
            source_id: doc.source_id,
            source_type: doc.source_type,
            source_label: doc.source_label,
            abs_path: PathBuf::from(doc.path),
            rel_path: doc.rel_path,
            force: true,
        }))
        .await;
    Ok(())
}

pub(crate) fn ensure_shelf_path(ctx: &Ctx, path: &str) -> CmdResult<()> {
    if !crate::core::read_lock(&ctx.library).allows_open_path(std::path::Path::new(path)) {
        return Err("That file is not in a Shelf Rebost knows.".into());
    }
    Ok(())
}

#[tauri::command]
pub fn open_original(app: AppHandle, ctx: State<'_, Arc<Ctx>>, path: String) -> CmdResult<()> {
    ensure_shelf_path(&ctx, &path)?;
    app.opener()
        .open_path(path, None::<String>)
        .map_err(friendly)
}

#[tauri::command]
pub fn reveal_item(app: AppHandle, ctx: State<'_, Arc<Ctx>>, path: String) -> CmdResult<()> {
    ensure_shelf_path(&ctx, &path)?;
    app.opener().reveal_item_in_dir(path).map_err(friendly)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::NoopEvents;
    use crate::ingest::extract::ExtractorSettings;
    use crate::paths::Paths;
    use std::sync::Arc;

    fn test_ctx() -> (tempfile::TempDir, Arc<Ctx>) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("appdata"));
        let ctx = Ctx::new(paths, Arc::new(NoopEvents), ExtractorSettings::default()).unwrap();
        (dir, ctx)
    }

    #[test]
    fn open_and_reveal_reject_paths_outside_a_shelf() {
        let (dir, ctx) = test_ctx();
        let root = dir.path().join("Rebost");
        let shelf = {
            let mut library = crate::core::write_lock(&ctx.library);
            library.create_shelf(&ctx.paths, "Legal", &root).unwrap()
        };
        let inside = shelf.managed_path.join("contract.md");
        std::fs::write(&inside, "x").unwrap();
        assert_eq!(ensure_shelf_path(&ctx, &inside.to_string_lossy()), Ok(()));
        assert_eq!(
            ensure_shelf_path(&ctx, "/tmp/not-in-a-shelf.md"),
            Err("That file is not in a Shelf Rebost knows.".into())
        );
    }

    #[test]
    fn document_commands_reject_traversal_ids() {
        assert!(require_id("../etc").is_err());
        assert!(require_id("s_ok").is_ok());
    }
}
