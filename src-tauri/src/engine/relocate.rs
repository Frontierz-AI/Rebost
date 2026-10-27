//! Choosing another folder for AI models, moving the installed ones there.
//!
//! The engine is stopped while files move (llama-server maps the weights, and
//! Windows won't remove an open file), then started again from the new folder.

use anyhow::{anyhow, Result};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{download, Engine};
use crate::models_folder::{self, Preflight, Relation};

/// Id of the move in the download controls, so `download_cancel` stops it.
pub const MOVE_ID: &str = "move:models";

/// Progress events at most this often while bytes move.
const PROGRESS_GAP: Duration = Duration::from_millis(120);

impl Engine {
    /// What choosing `to` would involve, without changing anything.
    pub fn models_folder_preflight(&self, to: &std::path::Path) -> Preflight {
        let files = crate::core::read_lock(&self.ctx.settings).model_files();
        Preflight::check(
            &self.ctx.paths,
            to,
            &files,
            &self.ctx.models_dir(),
            self.ctx.models_folder_missing(),
        )
    }

    /// Keep AI models in `chosen` (`None`: the default folder). Installed AIs
    /// move there first under the rule in [`models_folder`]; a failure or
    /// cancel leaves them where they were. Returns how many old copies were
    /// kept because the rule couldn't prove them redundant. Progress arrives
    /// as `rebost://models-move` events.
    pub async fn set_models_folder(self: &Arc<Self>, chosen: Option<PathBuf>) -> Result<usize> {
        let _install = self
            .install_lock
            .try_lock()
            .map_err(|_| anyhow!("ai-busy"))?;
        let default = self.ctx.paths.default_models_dir();
        let chosen = chosen.filter(|dir| models_folder::relation(dir, &default) != Relation::Same);
        let to = self.ctx.paths.models_dir_for(chosen.as_deref());
        let from = self.ctx.models_dir();
        // Places, not spellings: the current folder under another path is no
        // change. (A folder inside it, or holding it, fails the preflight.)
        if models_folder::relation(&to, &from) == Relation::Same {
            return Ok(0);
        }
        let preflight = self.models_folder_preflight(&to);
        if let Some(problem) = preflight.problem {
            return Err(anyhow!(problem.code()));
        }
        let plan = preflight.plan;
        let restart = self.active_model().is_some();
        let control = download::DownloadControl::new();
        crate::core::mutex_lock(&self.downloads).insert(MOVE_ID.into(), control.clone());
        let result = async {
            // Holding the start lock keeps Chat from starting the engine on
            // the old files mid-move; it waits, as the confirm says.
            let _no_start = if plan.to_move.is_empty() {
                None
            } else {
                self.wait_until_chat_idle().await;
                let no_start = self.start_lock.lock().await;
                self.stop().await;
                Some(no_start)
            };
            let events = self.ctx.events.clone();
            let staging = plan.clone();
            let journal = models_folder::journal_path(self.ctx.paths.base());
            let mover = tokio::task::spawn_blocking(move || {
                if staging.to_move.is_empty() {
                    return Ok(None);
                }
                let mut last: Option<Instant> = None;
                let mut progress = |done: u64, total: u64| {
                    if done < total && last.is_some_and(|at| at.elapsed() < PROGRESS_GAP) {
                        return;
                    }
                    last = Some(Instant::now());
                    events.emit(
                        "rebost://models-move",
                        json!({ "done": done, "total": total }),
                    );
                };
                models_folder::stage(&staging, &journal, &mut progress, &|| {
                    control.check_cancelled().is_err()
                })
                .map(Some)
            })
            .await??;
            let missing = plan.missing.clone();
            let saved = self.ctx.update_settings(|settings| {
                settings.models_folder = chosen.clone();
                forget_files(settings, &missing);
            });
            let Some(mover) = mover else {
                return saved.map(|()| 0);
            };
            if let Err(error) = saved {
                tokio::task::spawn_blocking(move || mover.roll_back()).await?;
                return Err(error);
            }
            let kept = tokio::task::spawn_blocking(move || mover.finish()).await?;
            if kept > 0 {
                log::warn!("kept {kept} old AI file(s) in {}", from.display());
            }
            Ok(kept)
        }
        .await;
        crate::core::mutex_lock(&self.downloads).remove(MOVE_ID);
        if let Err(error) = &result {
            log::error!("moving AI models to {} failed: {error:#}", to.display());
        }
        if restart {
            let engine = self.clone();
            tokio::spawn(async move {
                if let Err(error) = engine.ensure_ready().await {
                    log::warn!("engine start after choosing a models folder: {error:#}");
                }
            });
        } else if self.active_model().is_none() {
            self.set_status(super::EngineState::NoModel, None);
        }
        result
    }
}

/// Forget installed AIs whose weights are in neither folder. When that is
/// the active AI, the next installed one takes over.
fn forget_files(settings: &mut crate::settings::Settings, missing: &[String]) {
    if missing.is_empty() {
        return;
    }
    let gone = |file: &String| missing.contains(file);
    settings.other_models.retain(|model| !gone(&model.file));
    for model in settings
        .active_model
        .iter_mut()
        .chain(&mut settings.other_models)
    {
        if model.projector.as_ref().is_some_and(|p| gone(&p.file)) {
            model.projector = None;
        }
    }
    if settings
        .active_model
        .as_ref()
        .is_some_and(|m| gone(&m.file))
    {
        settings.active_model = if settings.other_models.is_empty() {
            None
        } else {
            Some(settings.other_models.remove(0))
        };
        settings.benchmark = None;
        settings.context_budget_chars = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Ctx;
    use crate::ingest::extract::ExtractorSettings;
    use crate::paths::Paths;
    use crate::settings::{InstalledModel, Settings, VisionProjector};
    use std::path::Path;

    #[derive(Default)]
    struct Moves(std::sync::Mutex<Vec<serde_json::Value>>);

    impl crate::core::Events for Moves {
        fn emit(&self, event: &str, payload: serde_json::Value) {
            if event == "rebost://models-move" {
                self.0.lock().unwrap().push(payload);
            }
        }
    }

    /// An engine whose installed AIs are not in use, so nothing starts.
    fn engine_with(dir: &Path, settings: Settings) -> (Arc<Engine>, Arc<Moves>) {
        let paths = Paths::new(dir.join("appdata"));
        paths.ensure().unwrap();
        settings.save(&paths.settings_path()).unwrap();
        let moves = Arc::new(Moves::default());
        let ctx = Ctx::new(paths, moves.clone(), ExtractorSettings::default()).unwrap();
        (Engine::new(ctx), moves)
    }

    fn write(dir: &Path, file: &str, bytes: &[u8]) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(file), bytes).unwrap();
    }

    fn model(file: &str) -> InstalledModel {
        InstalledModel {
            projector: None,
            file: file.into(),
            name: file.into(),
            source: "huggingface".into(),
            reference: format!("org/{file}"),
            license: None,
            size_bytes: 1,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn installed_ais_move_to_the_chosen_folder_and_back() {
        let dir = tempfile::tempdir().unwrap();
        let default = dir.path().join("appdata").join("models");
        let mut other = model("a.gguf");
        other.projector = Some(VisionProjector {
            file: "vision-1.gguf".into(),
            size_bytes: 2,
        });
        write(&default, "a.gguf", b"weights");
        write(&default, "vision-1.gguf", b"vv");
        write(&default, "notes.txt", b"not an AI");
        let settings = Settings {
            other_models: vec![other],
            ..Default::default()
        };
        let (engine, moves) = engine_with(dir.path(), settings);
        let drive = dir.path().join("Drive");
        std::fs::create_dir(&drive).unwrap();

        engine.set_models_folder(Some(drive.clone())).await.unwrap();
        assert_eq!(engine.ctx().models_dir(), drive);
        assert_eq!(std::fs::read(drive.join("a.gguf")).unwrap(), b"weights");
        assert!(drive.join("vision-1.gguf").is_file());
        assert!(!default.join("a.gguf").exists());
        assert!(
            default.join("notes.txt").is_file(),
            "only installed AIs move"
        );
        let saved = Settings::load(&engine.ctx().paths.settings_path());
        assert_eq!(saved.models_folder, Some(drive.clone()));
        let last = moves.0.lock().unwrap().last().cloned().unwrap();
        assert_eq!(last["done"], last["total"]);
        assert_eq!(last["total"], 9);

        engine.set_models_folder(None).await.unwrap();
        assert_eq!(engine.ctx().models_dir(), default);
        assert!(default.join("a.gguf").is_file());
        assert!(!drive.join("a.gguf").exists());
        assert_eq!(
            crate::core::read_lock(&engine.ctx().settings)
                .other_models
                .len(),
            1
        );
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn picking_the_current_folder_by_another_path_moves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let default = dir.path().join("appdata").join("models");
        write(&default, "a.gguf", b"only copy");
        let settings = Settings {
            other_models: vec![model("a.gguf")],
            ..Default::default()
        };
        let (engine, _) = engine_with(dir.path(), settings);
        let alias = dir.path().join("Alias");
        std::os::unix::fs::symlink(&default, &alias).unwrap();

        engine.set_models_folder(Some(alias)).await.unwrap();
        assert_eq!(std::fs::read(default.join("a.gguf")).unwrap(), b"only copy");
        assert_eq!(engine.ctx().models_dir(), default);
        assert_eq!(
            Settings::load(&engine.ctx().paths.settings_path()).models_folder,
            None
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_failed_move_leaves_everything_where_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let default = dir.path().join("appdata").join("models");
        write(&default, "a.gguf", b"weights");
        write(&default, "b.gguf", b"other weights");
        let settings = Settings {
            other_models: vec![model("a.gguf"), model("b.gguf")],
            ..Default::default()
        };
        let (engine, _) = engine_with(dir.path(), settings);
        let drive = dir.path().join("Drive");
        // Same size, other bytes: only the byte-for-byte check at move time sees it.
        write(&drive, "b.gguf", b"different 13!");

        let error = engine
            .set_models_folder(Some(drive.clone()))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("models-folder-name-taken"));
        assert_eq!(engine.ctx().models_dir(), default);
        assert!(default.join("a.gguf").is_file() && default.join("b.gguf").is_file());
        assert!(!drive.join("a.gguf").exists(), "the staged copy is removed");
        assert_eq!(
            std::fs::read(drive.join("b.gguf")).unwrap(),
            b"different 13!"
        );
        assert_eq!(
            Settings::load(&engine.ctx().paths.settings_path()).models_folder,
            None
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_unplugged_folder_forgets_nothing_until_the_default_is_chosen() {
        let dir = tempfile::tempdir().unwrap();
        let unplugged = dir.path().join("Unplugged");
        let settings = Settings {
            models_folder: Some(unplugged.clone()),
            other_models: vec![model("b.gguf")],
            ..Default::default()
        };
        let (engine, _) = engine_with(dir.path(), settings);
        let ctx = engine.ctx().clone();
        assert!(ctx.models_folder_missing());
        let error = ctx.ready_models_dir().unwrap_err();
        assert!(error.to_string().contains(models_folder::MISSING));
        assert!(!unplugged.exists(), "a chosen folder is never created");
        assert_eq!(crate::core::read_lock(&ctx.settings).other_models.len(), 1);
        for result in [
            engine.remove_model("b.gguf").await,
            engine.use_model("b.gguf").await,
        ] {
            assert!(result
                .unwrap_err()
                .to_string()
                .contains(models_folder::MISSING));
        }
        assert_eq!(crate::core::read_lock(&ctx.settings).other_models.len(), 1);

        // An empty folder would only forget them.
        let empty = dir.path().join("Empty");
        std::fs::create_dir(&empty).unwrap();
        let error = engine.set_models_folder(Some(empty)).await.unwrap_err();
        assert!(error.to_string().contains("models-folder-no-ais"));

        // The drive came back under another name: adopt the files in place.
        let renamed = dir.path().join("Renamed");
        write(&renamed, "b.gguf", b"weights");
        engine
            .set_models_folder(Some(renamed.clone()))
            .await
            .unwrap();
        assert_eq!(ctx.models_dir(), renamed);
        assert!(!ctx.models_folder_missing());
        assert_eq!(crate::core::read_lock(&ctx.settings).other_models.len(), 1);

        // Unplugged again; going back to Rebost's folder forgets what it can't reach.
        std::fs::remove_dir_all(&renamed).unwrap();
        engine.set_models_folder(None).await.unwrap();
        assert!(!ctx.models_folder_missing());
        assert!(crate::core::read_lock(&ctx.settings)
            .other_models
            .is_empty());
    }

    #[test]
    fn missing_files_are_forgotten_and_the_next_ai_takes_over() {
        let mut active = model("a.gguf");
        active.projector = Some(VisionProjector {
            file: "vision-1.gguf".into(),
            size_bytes: 1,
        });
        let mut settings = Settings {
            active_model: Some(active),
            other_models: vec![model("b.gguf"), model("c.gguf")],
            context_budget_chars: Some(9),
            ..Default::default()
        };
        forget_files(&mut settings, &[]);
        assert_eq!(settings.context_budget_chars, Some(9));

        forget_files(&mut settings, &["vision-1.gguf".into()]);
        assert!(settings.active_model.as_ref().unwrap().projector.is_none());
        assert_eq!(settings.context_budget_chars, Some(9));

        forget_files(&mut settings, &["a.gguf".into(), "c.gguf".into()]);
        assert_eq!(settings.active_model.as_ref().unwrap().file, "b.gguf");
        assert!(settings.other_models.is_empty());
        assert_eq!(settings.context_budget_chars, None);

        forget_files(&mut settings, &["b.gguf".into()]);
        assert!(settings.active_model.is_none());
    }
}
