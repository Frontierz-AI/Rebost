//! Engine status and model install commands.

use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use super::{friendly, CmdResult};
use crate::core::Ctx;
use crate::engine::models::{self, MachineProfile, ModelSearchResult, Recommendation};
use crate::engine::{Engine, EngineStatus};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineView {
    pub profile: MachineProfile,
    pub recommendation: Recommendation,
    pub alternatives: Vec<Recommendation>,
    /// Catalog picks that fit and are not installed (max two).
    pub suggestions: Vec<Recommendation>,
}

/// Current llama-server state.
#[tauri::command]
pub fn engine_status(engine: State<'_, Arc<Engine>>) -> EngineStatus {
    engine.status()
}

#[tauri::command]
pub async fn model_vision_offer(engine: State<'_, Arc<Engine>>) -> CmdResult<Option<u64>> {
    engine.vision_offer().await.map_err(super::friendly)
}

#[tauri::command]
pub async fn model_enable_vision(engine: State<'_, Arc<Engine>>) -> CmdResult<()> {
    engine
        .inner()
        .clone()
        .enable_vision()
        .await
        .map_err(super::friendly)
}

/// Hardware profile plus catalog recommendations.
#[tauri::command]
pub fn machine_profile(ctx: State<'_, Arc<Ctx>>) -> MachineView {
    let profile = MachineProfile::detect(&ctx.models_dir());
    let recommendation = models::recommend(&profile);
    let alternatives = models::smaller_alternatives(&profile, 2);
    let installed: Vec<String> = {
        let settings = crate::core::read_lock(&ctx.settings);
        settings
            .active_model
            .iter()
            .chain(&settings.other_models)
            .map(|model| model.reference.clone())
            .collect()
    };
    let installed: Vec<&str> = installed.iter().map(String::as_str).collect();
    let suggestions = models::uninstalled_suggestions(&profile, &installed, 2);
    MachineView {
        profile,
        recommendation,
        alternatives,
        suggestions,
    }
}

/// Search Hugging Face and Ollama catalogs for GGUF models.
#[tauri::command]
pub async fn models_search(
    ctx: State<'_, Arc<Ctx>>,
    engine: State<'_, Arc<Engine>>,
    query: String,
) -> CmdResult<Vec<ModelSearchResult>> {
    let profile = MachineProfile::detect(&ctx.models_dir());
    models::search_models(engine.catalog_client(), &query, &profile)
        .await
        .map_err(|_| {
            "Couldn't reach the AI catalogs. Check your connection and try again.".to_string()
        })
}

/// Open the public Hugging Face or Ollama page for a catalog AI.
#[tauri::command]
pub async fn open_model_page(app: AppHandle, source: String, reference: String) -> CmdResult<()> {
    let url = models::catalog_page_url(&source, &reference)
        .map_err(|_| "That page isn't available.".to_string())?;
    crate::host::open(&app, url).await
}

/// Install a model (the recommended one or a chosen one). Progress arrives
/// via `rebost://download` events.
#[tauri::command]
pub fn model_install(
    engine: State<'_, Arc<Engine>>,
    source: String,
    reference: String,
    name: String,
    license: Option<String>,
) -> CmdResult<()> {
    models::normalize_source(&source)
        .and_then(|source| models::validate_reference(source, &reference))
        .map_err(friendly)?;
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = engine
            .install_model(&source, &reference, &name, license)
            .await
        {
            log::error!("model install failed: {error:#}");
            let message = error.to_string();
            let error = if message.contains("stalled") {
                "stalled"
            } else if message.contains("SHA-256") || message.contains("verification") {
                "verification failed"
            } else if message.contains("incompatible-format") {
                "incompatible-format"
            } else if message.contains("invalid") || message.contains("unsupported model") {
                "That AI source isn't allowed."
            } else if message.contains("switch-failed") {
                "switch-failed"
            } else if message.contains("warmup-failed") {
                "warmup-failed"
            } else if message.contains("models-folder-missing") {
                "models-folder-missing"
            } else if message.contains("models-folder-full") {
                "models-folder-full"
            } else {
                "The download didn't finish. Try again."
            };
            engine.ctx().events.emit(
                "rebost://download",
                json!({
                    "kind": "model",
                    "id": format!("model:{reference}"),
                    "name": name,
                    "done": false,
                    "error": error,
                }),
            );
        }
    });
    Ok(())
}

/// Switch Chat to another installed AI. Resolves once it is Ready.
#[tauri::command]
pub async fn model_use(engine: State<'_, Arc<Engine>>, file: String) -> CmdResult<()> {
    engine
        .inner()
        .clone()
        .use_model(&file)
        .await
        .map_err(friendly)
}

/// Delete an installed AI that is not in use.
#[tauri::command]
pub async fn model_remove(engine: State<'_, Arc<Engine>>, file: String) -> CmdResult<()> {
    engine.remove_model(&file).await.map_err(friendly)
}

/// Show an installed AI's file in Finder or Explorer, or open the AI folder
/// when no file is given. Only installed file names are accepted.
#[tauri::command]
pub async fn model_reveal(
    app: AppHandle,
    ctx: State<'_, Arc<Ctx>>,
    file: Option<String>,
) -> CmdResult<()> {
    let dir = ctx.ready_models_dir().map_err(friendly)?;
    let Some(file) = file else {
        return crate::host::open(&app, dir.to_string_lossy().to_string()).await;
    };
    let installed = {
        let settings = crate::core::read_lock(&ctx.settings);
        settings
            .active_model
            .iter()
            .chain(&settings.other_models)
            .any(|model| model.file == file)
    };
    let path = dir.join(&file);
    if !installed || !path.is_file() {
        return Err(friendly("ai-missing"));
    }
    app.opener().reveal_item_in_dir(path).map_err(friendly)
}

/// Cancel an in-flight model download.
#[tauri::command]
pub fn download_cancel(engine: State<'_, Arc<Engine>>, id: String) {
    engine.cancel_download(&id);
}

/// Skip SHA-256 check for a model download that stalled on verify.
#[tauri::command]
pub fn download_skip_verify(engine: State<'_, Arc<Engine>>, id: String) {
    if id.starts_with("model:") || id.starts_with("vision:") {
        engine.skip_download_verify(&id);
    }
}

/// Recalibrate the installed AI on the active engine.
#[tauri::command]
pub async fn engine_remeasure(engine: State<'_, Arc<Engine>>) -> CmdResult<()> {
    engine.inner().clone().remeasure().await.map_err(friendly)
}

/// The folder picked in the last `models_folder_pick`, applied by
/// `models_folder_apply`. The UI never sends a path.
#[derive(Default)]
pub struct PendingModelsFolder(Mutex<Option<PathBuf>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsFolderChoice {
    pub path: String,
    /// Rebost's own folder was picked.
    pub is_default: bool,
    /// Already the folder in use; nothing to do.
    pub unchanged: bool,
    pub free_bytes: Option<u64>,
    /// Bytes of installed AIs that would move.
    pub move_bytes: u64,
    /// Why this folder can't be used, as an error code.
    pub problem: Option<&'static str>,
}

/// Ask for a folder to keep AIs in, and say what using it would involve.
#[tauri::command]
pub async fn models_folder_pick(
    app: AppHandle,
    engine: State<'_, Arc<Engine>>,
    pending: State<'_, PendingModelsFolder>,
) -> CmdResult<Option<ModelsFolderChoice>> {
    let ctx = engine.ctx();
    let mut dialog = app.dialog().file();
    if let Some(start) = ctx.models_dir().ancestors().find(|dir| dir.is_dir()) {
        dialog = dialog.set_directory(start);
    }
    let Some(folder) = dialog.blocking_pick_folder() else {
        return Ok(None);
    };
    let folder: PathBuf = folder.into_path().map_err(friendly)?;
    let folder = crate::models_folder::plain_path(folder.canonicalize().unwrap_or(folder));
    use crate::models_folder::{relation, Relation};
    let preflight = engine.models_folder_preflight(&folder);
    let choice = ModelsFolderChoice {
        path: folder.to_string_lossy().into_owned(),
        is_default: relation(&folder, &ctx.paths.default_models_dir()) == Relation::Same,
        unchanged: relation(&folder, &ctx.models_dir()) == Relation::Same,
        free_bytes: preflight.free,
        move_bytes: preflight.plan.bytes(),
        problem: preflight.problem.map(|problem| problem.code()),
    };
    *crate::core::mutex_lock(&pending.0) = Some(folder);
    Ok(Some(choice))
}

/// Keep AIs in the folder picked last, moving installed ones there.
/// Progress arrives as `rebost://models-move`; `download_cancel` with
/// `move:models` stops it and leaves everything where it was.
/// How a models-folder change ended.
#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MoveOutcome {
    Done,
    Cancelled,
    /// In the new folder, but some old copies were kept (the rule couldn't
    /// prove them redundant).
    KeptCopies,
}

#[tauri::command]
pub async fn models_folder_apply(
    engine: State<'_, Arc<Engine>>,
    pending: State<'_, PendingModelsFolder>,
) -> CmdResult<MoveOutcome> {
    let folder = crate::core::mutex_lock(&pending.0)
        .take()
        .ok_or_else(|| friendly(crate::models_folder::MISSING))?;
    finished_move(engine.inner().clone().set_models_folder(Some(folder)).await)
}

fn finished_move(result: anyhow::Result<usize>) -> CmdResult<MoveOutcome> {
    match result {
        Ok(0) => Ok(MoveOutcome::Done),
        Ok(_) => Ok(MoveOutcome::KeptCopies),
        Err(error) if error.to_string() == "cancelled" => Ok(MoveOutcome::Cancelled),
        Err(error) => Err(friendly(error)),
    }
}

/// Go back to Rebost's own folder, moving installed AIs back when they can
/// be reached. While the chosen folder is unavailable, the AIs in it are
/// forgotten (their files are not touched).
#[tauri::command]
pub async fn models_folder_use_default(engine: State<'_, Arc<Engine>>) -> CmdResult<MoveOutcome> {
    finished_move(engine.inner().clone().set_models_folder(None).await)
}

/// Look again for a chosen folder that was unavailable (a drive plugged
/// back in) and start the AI when it is back.
#[tauri::command]
pub async fn models_folder_recheck(engine: State<'_, Arc<Engine>>) -> CmdResult<bool> {
    if engine.ctx().models_folder_missing() {
        return Ok(false);
    }
    if engine.status().state == crate::engine::EngineState::NoModel
        && crate::core::read_lock(&engine.ctx().settings)
            .active_model
            .is_some()
    {
        engine
            .inner()
            .clone()
            .ensure_ready()
            .await
            .map_err(friendly)?;
    }
    Ok(true)
}
