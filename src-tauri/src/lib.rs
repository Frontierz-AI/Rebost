//! Rebost: Private AI that lives with your files and never leaves them.
//!
//! ```text
//! UI (Svelte) ──invoke──► commands ──► shelf / ingest / search / chat / engine
//! ```
//!
//! See `docs/architecture.md` in the repository root.

pub mod about;
pub mod chat;
pub mod commands;
pub mod core;
pub mod engine;
pub mod ids;
pub mod ingest;
pub mod paths;
pub mod pii;
pub mod recipes;
pub mod reset;
pub mod search;
pub mod settings;
pub mod shelf;
pub mod types;
pub mod updater;

use std::sync::{Arc, Mutex};
use tauri::Manager;

use crate::chat::ChatService;
use crate::core::{Ctx, Events};
use crate::engine::{Engine, EngineState};
use crate::ingest::extract::ExtractorSettings;
use crate::ingest::Ingestor;
use crate::paths::Paths;
use crate::shelf::watcher::WatcherHub;

/// Forwards core events to the webview.
struct TauriEvents(tauri::AppHandle);

impl Events for TauriEvents {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        use tauri::Emitter;
        if let Err(error) = self.0.emit(event, payload) {
            log::warn!("emit {event}: {error}");
        }
    }
}

/// Copy bundled `*.traineddata` into app data where extraction expects it.
fn provision_tessdata(app: &tauri::App, data_dir: &std::path::Path) -> std::path::PathBuf {
    let target = data_dir.join("engine").join("tessdata");
    std::fs::create_dir_all(&target).ok();
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("resources").join("tessdata"));
    }
    // Development fallback: straight from the source tree.
    candidates.push(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("tessdata"),
    );
    for candidate in &candidates {
        let Ok(entries) = std::fs::read_dir(candidate) else {
            continue;
        };
        for entry in entries.flatten() {
            let source = entry.path();
            let Some(name) = source.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.ends_with(".traineddata") {
                continue;
            }
            let dest = target.join(name);
            if dest.exists() {
                continue;
            }
            if let Err(error) = std::fs::copy(&source, &dest) {
                log::warn!("copying {name}: {error}");
            }
        }
    }
    target
}

fn find_bundled_engine_archive(app: &tauri::App) -> Option<std::path::PathBuf> {
    let pin = crate::engine::current_engine_pin().ok()?;
    let mut roots = Vec::new();
    if let Ok(resource_dir) = app.path().resource_dir() {
        roots.push(resource_dir.join("resources").join("engine"));
        roots.push(resource_dir.join("engine"));
    }
    roots.push(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("engine"),
    );
    crate::engine::find_bundled_engine_archive(roots, pin.file_name)
}

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("rebost".into()),
                    }),
                ])
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .on_menu_event(|app, event| crate::about::on_menu_event(app, &event))
        .menu(crate::about::build_menu);

    builder
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("app data dir must resolve");
            let identifier = app.config().identifier.clone();
            let extras = crate::reset::extra_paths(
                &identifier,
                app.path().app_cache_dir().ok(),
                app.path().app_log_dir().ok(),
                app.path().app_local_data_dir().ok(),
            );
            match crate::reset::apply_pending(&data_dir, &extras, &identifier) {
                Ok(true) => log::info!("workspace reset to first-run"),
                Ok(false) => {}
                Err(error) => log::error!("workspace reset failed: {error}"),
            }
            let tessdata_dir = provision_tessdata(app, &data_dir);
            let mut paths = Paths::new(&data_dir);
            paths.set_bundled_engine_archive(find_bundled_engine_archive(app));
            let events: Arc<dyn Events> = Arc::new(TauriEvents(app.handle().clone()));
            let ctx = Ctx::new(
                paths,
                events,
                ExtractorSettings {
                    tessdata_dir: Some(tessdata_dir),
                    timeout_secs: 300,
                },
            )?;
            let engine = Engine::new(ctx.clone());
            let chat = ChatService::new(ctx.clone(), engine.clone());

            // Core workers live on the tokio runtime tauri drives.
            let runtime_guard = tauri::async_runtime::handle().inner().clone();
            let _enter = runtime_guard.enter();
            let ingestor = Ingestor::start(ctx.clone());
            let watcher = WatcherHub::start(ctx.clone(), ingestor.clone());

            // Startup: bring every Shelf source in sync (new/changed/removed
            // files while Rebost was closed).
            {
                let ingestor = ingestor.clone();
                tauri::async_runtime::spawn(async move {
                    ingestor.sync_all(false).await;
                });
            }

            if engine.status().state != EngineState::NoModel {
                let engine = engine.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = engine.ensure_ready().await {
                        log::error!("startup warmup failed: {error:#}");
                    }
                });
            }

            app.manage(crate::updater::PendingUpdate(Mutex::new(None)));
            app.manage(ctx);
            app.manage(engine);
            app.manage(chat);
            app.manage(ingestor);
            app.manage(watcher);

            let updater_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                crate::updater::check_silently(updater_handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::shelves_list,
            commands::shelf_create,
            commands::shelf_remove,
            commands::shelf_add_linked,
            commands::shelf_remove_source,
            commands::shelf_import_paths,
            commands::shelf_import_dialog,
            commands::shelf_documents,
            commands::document_card,
            commands::document_text,
            commands::document_reprocess,
            commands::open_original,
            commands::reveal_item,
            commands::threads_list,
            commands::thread_create,
            commands::thread_messages,
            commands::thread_set_shelf,
            commands::thread_delete,
            commands::chat_send,
            commands::chat_cancel,
            commands::warm_engine,
            commands::engine_status,
            commands::machine_profile,
            commands::active_model,
            commands::models_search,
            commands::model_install,
            commands::download_cancel,
            commands::download_skip_verify,
            commands::settings_get,
            commands::settings_set_house_rules,
            commands::settings_finish_onboarding,
            commands::settings_reset_workspace,
            commands::redact_text,
            commands::text_has_pii,
            commands::diagnostics,
            commands::recipes_list,
            commands::recipe_create,
            commands::recipe_delete,
            commands::recipes_restore_defaults,
            about::about_info,
            about::show_about_window,
            about::open_external,
            updater::update_info,
            updater::install_update,
            updater::show_update_window,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Rebost")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                // Closing Rebost stops the engine and releases its memory.
                if let Some(engine) = app.try_state::<Arc<Engine>>() {
                    engine.stop_blocking();
                }
            }
        });
}
