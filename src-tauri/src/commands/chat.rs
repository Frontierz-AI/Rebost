//! Thread and chat commands.

use std::sync::Arc;
use tauri::State;

use super::{friendly, require_id, require_optional_id, CmdResult};
use crate::chat::conversations::{Conversations, StoredMessage, ThreadMeta};
use crate::chat::ChatService;
use crate::core::Ctx;
use crate::engine::Engine;

#[tauri::command]
pub fn threads_list(ctx: State<'_, Arc<Ctx>>) -> Vec<ThreadMeta> {
    Conversations::list(&ctx.paths)
}

#[tauri::command]
pub fn thread_create(ctx: State<'_, Arc<Ctx>>, shelf_id: Option<String>) -> CmdResult<ThreadMeta> {
    require_optional_id(shelf_id.as_deref())?;
    Conversations::create(&ctx.paths, shelf_id).map_err(friendly)
}

#[tauri::command]
pub fn thread_messages(
    ctx: State<'_, Arc<Ctx>>,
    thread_id: String,
) -> CmdResult<Vec<StoredMessage>> {
    require_id(&thread_id)?;
    Ok(Conversations::messages(&ctx.paths, &thread_id))
}

#[tauri::command]
pub fn thread_set_shelf(
    ctx: State<'_, Arc<Ctx>>,
    thread_id: String,
    shelf_id: Option<String>,
) -> CmdResult<()> {
    require_id(&thread_id)?;
    require_optional_id(shelf_id.as_deref())?;
    Conversations::set_shelf(&ctx.paths, &thread_id, shelf_id).map_err(friendly)
}

#[tauri::command]
pub fn thread_delete(ctx: State<'_, Arc<Ctx>>, thread_id: String) -> CmdResult<()> {
    require_id(&thread_id)?;
    Conversations::delete(&ctx.paths, &thread_id).map_err(friendly)?;
    ctx.search.remove_thread(&thread_id).ok();
    Ok(())
}

/// Send a message — returns immediately; `rebost://chat` events stream the
/// answer (queued while Warming up…).
#[tauri::command]
pub fn chat_send(
    chat: State<'_, Arc<ChatService>>,
    thread_id: String,
    text: String,
    shelf_id: Option<String>,
) -> CmdResult<()> {
    require_id(&thread_id)?;
    require_optional_id(shelf_id.as_deref())?;
    let chat = chat.inner().clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = chat.send_message(&thread_id, &text, shelf_id).await {
            log::error!("chat send failed: {error:#}");
        }
    });
    Ok(())
}

#[tauri::command]
pub fn chat_cancel(chat: State<'_, Arc<ChatService>>, message_id: String) -> CmdResult<()> {
    require_id(&message_id)?;
    chat.cancel(&message_id);
    Ok(())
}

/// The composer was focused — start the engine in the background so the
/// first answer needs no wait ("Warming up…" otherwise).
#[tauri::command]
pub fn warm_engine(engine: State<'_, Arc<Engine>>) {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn(async move {
        engine.ensure_ready().await.ok();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_and_cancel_reject_unsafe_ids() {
        assert!(require_id("../thread").is_err());
        assert!(require_id("a/b").is_err());
        assert!(require_optional_id(Some("..")).is_err());
        assert_eq!(require_optional_id(None), Ok(()));
        assert!(require_id("t_ab12cd").is_ok());
    }
}
