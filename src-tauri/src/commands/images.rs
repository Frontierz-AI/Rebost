//! Image intake stays separate from document extraction and Shelf indexing.

use super::{friendly, require_id, CmdResult};
use crate::{
    chat::images::{self, ChatImage},
    core::Ctx,
    engine::Engine,
};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn chat_image_add(
    ctx: State<'_, Arc<Ctx>>,
    engine: State<'_, Arc<Engine>>,
    pending: State<'_, super::shelves::PendingImports>,
    thread_id: String,
    name: String,
    data: Option<String>,
    path: Option<String>,
) -> CmdResult<ChatImage> {
    require_id(&thread_id)?;
    let limits = engine
        .vision_limits()
        .ok_or_else(|| friendly("image-unavailable"))?;
    let selected = match (path, data.as_ref()) {
        (Some(path), None) => pending
            .take_allowed(vec![path.into()])
            .pop()
            .ok_or_else(|| friendly("not allowed"))
            .map(Some)?,
        (None, Some(_)) => None,
        _ => return Err(friendly("image-invalid")),
    };
    let ctx = ctx.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = if let Some(path) = selected {
            if std::fs::metadata(&path)?.len() > images::MAX_IMAGE_BYTES as u64 {
                anyhow::bail!("image-too-large");
            }
            std::fs::read(path)?
        } else {
            images::decode_base64(data.as_deref().unwrap_or_default())?
        };
        images::store(&ctx.paths, &thread_id, &name, &bytes, limits)
    })
    .await
    .map_err(friendly)?
    .map_err(friendly)
}

#[tauri::command]
pub async fn chat_image_read(
    ctx: State<'_, Arc<Ctx>>,
    thread_id: String,
    image_id: String,
    thumbnail: bool,
) -> CmdResult<String> {
    let ctx = ctx.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        images::data_url(&ctx.paths, &thread_id, &image_id, thumbnail)
    })
    .await
    .map_err(friendly)?
    .map_err(friendly)
}

#[tauri::command]
pub fn chat_image_remove(
    ctx: State<'_, Arc<Ctx>>,
    thread_id: String,
    image_id: String,
) -> CmdResult<()> {
    images::remove_draft(&ctx.paths, &thread_id, &image_id).map_err(friendly)
}
