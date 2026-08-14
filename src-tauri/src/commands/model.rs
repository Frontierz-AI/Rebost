//! Engine status and model install commands.

use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use tauri::State;

use super::{friendly, CmdResult};
use crate::core::Ctx;
use crate::engine::models::{self, MachineProfile, ModelSearchResult, Recommendation};
use crate::engine::{Engine, EngineStatus};
use crate::settings::ActiveModel;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineView {
    pub profile: MachineProfile,
    pub recommendation: Recommendation,
    pub alternatives: Vec<Recommendation>,
    pub recommendation_fits: bool,
    /// Catalog picks that fit and are not the installed model (max two).
    pub suggestions: Vec<Recommendation>,
}

#[tauri::command]
pub fn engine_status(engine: State<'_, Arc<Engine>>) -> EngineStatus {
    engine.status()
}

#[tauri::command]
pub fn machine_profile(ctx: State<'_, Arc<Ctx>>) -> MachineView {
    let profile = MachineProfile::detect(ctx.paths.base());
    let recommendation = models::recommend(&profile);
    let alternatives = models::smaller_alternatives(&profile, 2);
    let installed = crate::core::read_lock(&ctx.settings)
        .active_model
        .as_ref()
        .map(|model| model.reference.clone());
    let suggestions = models::uninstalled_suggestions(&profile, installed.as_deref(), 2);
    let fits =
        models::runtime_need_bytes(recommendation.approx_bytes) <= profile.model_budget_bytes();
    MachineView {
        profile,
        recommendation,
        alternatives,
        recommendation_fits: fits,
        suggestions,
    }
}

#[tauri::command]
pub fn active_model(ctx: State<'_, Arc<Ctx>>) -> Option<ActiveModel> {
    crate::core::read_lock(&ctx.settings).active_model.clone()
}

#[tauri::command]
pub async fn models_search(
    ctx: State<'_, Arc<Ctx>>,
    engine: State<'_, Arc<Engine>>,
    query: String,
) -> CmdResult<Vec<ModelSearchResult>> {
    let profile = MachineProfile::detect(ctx.paths.base());
    models::search_models(engine.catalog_client(), &query, &profile)
        .await
        .map_err(|_| {
            "The model catalogs couldn't be reached. Check your connection and try again."
                .to_string()
        })
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
            } else if message.contains("invalid") || message.contains("unsupported model") {
                "That model source isn't allowed."
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

#[tauri::command]
pub fn download_cancel(engine: State<'_, Arc<Engine>>, id: String) {
    engine.cancel_download(&id);
}

#[tauri::command]
pub fn download_skip_verify(engine: State<'_, Arc<Engine>>, id: String) {
    if id.starts_with("model:") {
        engine.skip_download_verify(&id);
    }
}
