//! Settings, privacy helpers, and diagnostics.

use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, State};

use super::{friendly, CmdResult};
use crate::core::Ctx;
use crate::engine::models::MachineProfile;
use crate::engine::{Engine, EngineStatus};
use crate::settings::ActiveModel;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub house_rules: String,
    pub shelf_root: String,
    pub onboarding_done: bool,
    pub active_model: Option<ActiveModel>,
}

#[tauri::command]
pub fn settings_get(ctx: State<'_, Arc<Ctx>>) -> SettingsView {
    let settings = crate::core::read_lock(&ctx.settings);
    SettingsView {
        house_rules: settings.house_rules.clone(),
        shelf_root: settings.shelf_root().to_string_lossy().to_string(),
        onboarding_done: settings.onboarding_done,
        active_model: settings.active_model.clone(),
    }
}

#[tauri::command]
pub fn settings_set_house_rules(ctx: State<'_, Arc<Ctx>>, text: String) {
    crate::core::write_lock(&ctx.settings).house_rules = text;
    ctx.save_settings();
}

#[tauri::command]
pub fn settings_finish_onboarding(ctx: State<'_, Arc<Ctx>>) {
    crate::core::write_lock(&ctx.settings).onboarding_done = true;
    ctx.save_settings();
}

pub(crate) fn require_reset_confirmation(confirmation: &str) -> CmdResult<()> {
    if confirmation.trim() != crate::reset::CONFIRMATION {
        return Err("Type DELETE to confirm.".into());
    }
    Ok(())
}

/// Wipe app data on the next launch (same idea as `scripts/reset.sh`).
/// Requires typing DELETE. Shelf folders outside app data are kept.
#[tauri::command]
pub async fn settings_reset_workspace(
    app: AppHandle,
    ctx: State<'_, Arc<Ctx>>,
    engine: State<'_, Arc<Engine>>,
    confirmation: String,
) -> CmdResult<()> {
    require_reset_confirmation(&confirmation)?;
    crate::reset::mark_pending(ctx.paths.base()).map_err(friendly)?;
    engine.cancel_all_downloads();
    engine.stop().await;
    app.request_restart();
    Ok(())
}

/// "Copy without personal information" — local replacement, no history.
#[tauri::command]
pub fn redact_text(ctx: State<'_, Arc<Ctx>>, text: String) -> String {
    ctx.pii.redact(&text)
}

#[tauri::command]
pub fn text_has_pii(ctx: State<'_, Arc<Ctx>>, text: String) -> bool {
    ctx.pii.contains_pii(&text)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub version: String,
    pub data_dir: String,
    pub engine_build: String,
    pub engine_state: EngineStatus,
    pub model: Option<ActiveModel>,
    pub index_records: u64,
    pub context_budget_chars: usize,
    pub benchmark: Option<crate::settings::BenchmarkResult>,
    pub machine: MachineProfile,
    /// Path only. The log itself can name local files; it is not sent to the UI.
    pub engine_log_path: String,
    pub engine_log_present: bool,
    pub supported_formats: Vec<String>,
}

#[tauri::command]
pub fn diagnostics(
    app: AppHandle,
    ctx: State<'_, Arc<Ctx>>,
    engine: State<'_, Arc<Engine>>,
) -> Diagnostics {
    let settings = crate::core::read_lock(&ctx.settings).clone();
    let log_path = ctx.paths.logs_dir().join("engine.log");
    let mut formats: Vec<String> = crate::ingest::extract::supported_extensions()
        .iter()
        .cloned()
        .collect();
    formats.sort();
    Diagnostics {
        version: app.package_info().version.to_string(),
        data_dir: ctx.paths.base().to_string_lossy().to_string(),
        engine_build: crate::engine::ENGINE_BUILD.to_string(),
        engine_state: engine.status(),
        model: settings.active_model.clone(),
        index_records: ctx.search.num_docs(),
        context_budget_chars: ctx.context_budget(),
        benchmark: settings.benchmark.clone(),
        machine: MachineProfile::detect(ctx.paths.base()),
        engine_log_path: log_path.to_string_lossy().to_string(),
        engine_log_present: log_path.is_file(),
        supported_formats: formats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_confirmation_must_be_delete() {
        assert_eq!(require_reset_confirmation("DELETE"), Ok(()));
        assert_eq!(require_reset_confirmation("  DELETE  "), Ok(()));
        assert_eq!(
            require_reset_confirmation("delete"),
            Err("Type DELETE to confirm.".into())
        );
        assert_eq!(
            require_reset_confirmation(""),
            Err("Type DELETE to confirm.".into())
        );
    }
}
