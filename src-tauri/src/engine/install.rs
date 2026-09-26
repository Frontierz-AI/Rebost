//! Model download + switch-over.
//!
//! The previous AI stays usable while the new file downloads. Its file is
//! removed only after the new process is Ready. A failed start rolls back.

use anyhow::{anyhow, Result};
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use super::download;
use super::models;
use super::{Engine, EngineState};
use crate::settings::{ActiveModel, BenchmarkResult};

struct PreviousAi {
    model: ActiveModel,
    benchmark: Option<BenchmarkResult>,
    context_budget_chars: Option<usize>,
}

/// A model is installed only after all components have downloaded and the engine is ready.
struct InstallEvents {
    events: Arc<dyn crate::core::Events>,
    offset: u64,
    total: Option<u64>,
}
impl crate::core::Events for InstallEvents {
    fn emit(&self, event: &str, mut payload: serde_json::Value) {
        if event == "rebost://download" {
            payload["done"] = json!(false);
            if let Some(received) = payload["received"].as_u64() {
                payload["received"] = json!(self.offset.saturating_add(received));
            }
            if let Some(total) = self.total {
                payload["total"] = json!(total);
            }
        }
        self.events.emit(event, payload);
    }
}

impl Engine {
    pub fn cancel_download(&self, id: &str) {
        if let Some(control) = crate::core::mutex_lock(&self.downloads).get(id) {
            control.request_cancel();
        }
    }

    pub fn skip_download_verify(&self, id: &str) {
        if let Some(control) = crate::core::mutex_lock(&self.downloads).get(id) {
            control.request_skip_verify();
        }
    }

    pub fn cancel_all_downloads(&self) {
        for control in crate::core::mutex_lock(&self.downloads).values() {
            control.request_cancel();
        }
    }

    /// Download and switch to a model. Chat keeps the previous AI until the
    /// new process is Ready; only then is the old file removed.
    pub async fn install_model(
        self: &Arc<Self>,
        source: &str,
        reference: &str,
        display_name: &str,
        license: Option<String>,
    ) -> Result<()> {
        let _install = self.install_lock.lock().await;
        let source = models::normalize_source(source)?;
        models::validate_reference(source, reference)?;

        let ticket = download::DownloadTicket {
            kind: "model",
            id: format!("model:{reference}"),
            name: display_name.to_string(),
        };
        let control = download::DownloadControl::new();
        crate::core::mutex_lock(&self.downloads).insert(ticket.id.clone(), control.clone());
        let had_model = self.active_model().is_some();
        if !had_model {
            self.set_status(EngineState::Downloading, Some(display_name.to_string()));
        }
        self.ctx.events.emit(
            "rebost://download",
            json!({
                "kind": "model",
                "id": ticket.id,
                "name": display_name,
                "received": 0,
                "total": null,
                "done": false,
            }),
        );

        let result = self
            .download_and_switch(&ticket, &control, source, reference, display_name, license)
            .await;
        crate::core::mutex_lock(&self.downloads).remove(&ticket.id);
        if result.is_err() && self.active_model().is_none() {
            self.set_status(EngineState::NoModel, None);
        }
        if result.is_ok() {
            self.ctx.events.emit(
                "rebost://download",
                json!({ "kind": ticket.kind, "id": ticket.id, "name": ticket.name, "done": true }),
            );
        }
        result
    }

    async fn download_and_switch(
        self: &Arc<Self>,
        ticket: &download::DownloadTicket,
        control: &download::DownloadControl,
        source: &str,
        reference: &str,
        display_name: &str,
        license: Option<String>,
    ) -> Result<()> {
        let resolved = models::resolve_download(&self.download_client, source, reference).await?;
        let sha256 = resolved
            .sha256
            .as_deref()
            .ok_or_else(|| anyhow!("model file has no SHA-256 checksum; refusing to install"))?;
        let requested = models::safe_model_file_name(&resolved.file_name)?;
        let previous = snapshot_previous(self);
        let file_name =
            install_file_name(&requested, previous.as_ref().map(|p| p.model.file.as_str()));
        let dest = self.ctx.paths.models_dir().join(&file_name);

        let companion = self
            .eligible_projector(
                source,
                reference,
                &resolved.file_name,
                resolved.size.unwrap_or(0),
            )
            .await?;
        let total = resolved.size.map(|bytes| {
            bytes.saturating_add(companion.as_ref().and_then(|p| p.size).unwrap_or(0))
        });
        let events: Arc<dyn crate::core::Events> = Arc::new(InstallEvents {
            events: self.ctx.events.clone(),
            offset: 0,
            total,
        });
        download::download(
            &self.download_client,
            &resolved.url,
            &dest,
            ticket,
            Some(sha256),
            resolved.size,
            &events,
            control,
        )
        .await?;

        if let Err(error) = super::gguf::require_engine_compatible(&dest) {
            let _ = std::fs::remove_file(&dest);
            return Err(error);
        }

        let projector = if let Some(companion) = companion {
            let events: Arc<dyn crate::core::Events> = Arc::new(InstallEvents {
                events: self.ctx.events.clone(),
                offset: resolved.size.unwrap_or(0),
                total,
            });
            Some(
                self.download_projector(&companion, ticket, control, &events)
                    .await?,
            )
        } else {
            None
        };
        control.check_cancelled()?;

        self.ctx.events.emit("rebost://download", json!({ "kind": ticket.kind, "id": ticket.id, "name": ticket.name, "phase": "preparing", "done": false }));

        if previous.is_some() {
            self.wait_until_chat_idle().await;
        }
        control.check_cancelled()?;

        {
            let mut settings = crate::core::write_lock(&self.ctx.settings);
            settings.active_model = Some(ActiveModel {
                projector,
                file: file_name.clone(),
                name: display_name.to_string(),
                source: source.to_string(),
                reference: reference.to_string(),
                license,
                size_bytes: resolved.size.unwrap_or(0),
            });
            settings.benchmark = None;
            settings.context_budget_chars = None;
        }
        self.ctx.save_settings();
        *crate::core::mutex_lock(&self.vision_error) = None;

        match self.ensure_ready().await {
            Ok(_) => {
                if let Some(active) = self.active_model() {
                    if let Some(projector) = active
                        .projector
                        .as_ref()
                        .filter(|_| self.vision_start_failed(&active))
                    {
                        log::warn!(
                            "image support did not start with {}; removing it",
                            active.file
                        );
                        self.drop_failed_projector(&active.file, &projector.file);
                    }
                }
                if let Some(old) = previous_file_to_remove(
                    previous.as_ref().map(|p| p.model.file.as_str()),
                    &file_name,
                ) {
                    let _ = std::fs::remove_file(self.ctx.paths.models_dir().join(old));
                }
                if let Some(old) = previous.as_ref().and_then(|p| p.model.projector.as_ref()) {
                    if self
                        .active_model()
                        .and_then(|m| m.projector)
                        .is_none_or(|p| p.file != old.file)
                    {
                        let _ = std::fs::remove_file(self.ctx.paths.models_dir().join(&old.file));
                    }
                }
                Ok(())
            }
            Err(error) => {
                log::error!("engine warmup after install failed: {error:#}");
                if let Some(previous) = previous {
                    restore_previous(self, previous);
                    if let Err(restart) = self.ensure_ready().await {
                        log::error!("could not restore previous AI: {restart:#}");
                    }
                    Err(anyhow!("switch-failed"))
                } else {
                    clear_failed_first_install(self);
                    Err(anyhow!("warmup-failed"))
                }
            }
        }
    }

    async fn eligible_projector(
        &self,
        source: &str,
        reference: &str,
        model_file: &str,
        weights: u64,
    ) -> Result<Option<models::ResolvedDownload>> {
        let resolved =
            match models::resolve_projector(&self.download_client, source, reference, model_file)
                .await
            {
                Ok(Some(resolved)) => resolved,
                Ok(None) => return Ok(None),
                Err(error) => {
                    log::warn!("vision component lookup failed: {error}");
                    return Ok(None);
                }
            };
        let profile = super::catalog::MachineProfile::detect(self.ctx.paths.base());
        if super::vision::limits_for(&profile, weights, resolved.size.unwrap_or(0)).is_none()
            || !resolved.sha256.as_deref().is_some_and(valid_sha256)
        {
            return Ok(None);
        }
        models::safe_model_file_name(&resolved.file_name)?;
        Ok(Some(resolved))
    }

    async fn download_projector(
        &self,
        resolved: &models::ResolvedDownload,
        ticket: &download::DownloadTicket,
        control: &download::DownloadControl,
        events: &Arc<dyn crate::core::Events>,
    ) -> Result<crate::settings::VisionProjector> {
        let sha = resolved
            .sha256
            .as_deref()
            .filter(|sha| valid_sha256(sha))
            .ok_or_else(|| anyhow!("invalid projector checksum"))?;
        // Digest naming prevents collisions between repositories with mmproj-F16.gguf.
        let file = format!("vision-{sha}.gguf");
        let dest = self.ctx.paths.models_dir().join(&file);
        download::download(
            &self.download_client,
            &resolved.url,
            &dest,
            ticket,
            Some(sha),
            resolved.size,
            events,
            control,
        )
        .await?;
        super::gguf::require_engine_compatible(&dest)?;
        Ok(crate::settings::VisionProjector {
            file,
            size_bytes: resolved.size.unwrap_or(0),
        })
    }

    /// Image support was attempted with this AI and did not come up.
    fn vision_start_failed(&self, model: &ActiveModel) -> bool {
        model.projector.is_some()
            && self.vision_limits().is_none()
            && (crate::core::mutex_lock(&self.disabled_vision).as_deref() == Some(&model.file)
                || crate::core::mutex_lock(&self.vision_error).is_some())
    }

    /// Forget and delete an image file that would not start, and stop
    /// offering it for this AI on this engine release.
    fn drop_failed_projector(&self, model_file: &str, projector_file: &str) {
        {
            let mut settings = crate::core::write_lock(&self.ctx.settings);
            if let Some(active) = settings
                .active_model
                .as_mut()
                .filter(|active| active.file == model_file)
            {
                active.projector = None;
            }
            settings.vision_failed_for = Some(super::vision::failed_vision_key(model_file));
        }
        self.ctx.save_settings();
        let _ = std::fs::remove_file(self.ctx.paths.models_dir().join(projector_file));
    }

    /// Check whether the installed AI can gain vision without replacing its language weights.
    pub async fn vision_offer(&self) -> Result<Option<u64>> {
        let Some(model) = self
            .active_model()
            .filter(|model| model.projector.is_none())
        else {
            return Ok(None);
        };
        if crate::core::read_lock(&self.ctx.settings)
            .vision_failed_for
            .as_deref()
            == Some(super::vision::failed_vision_key(&model.file).as_str())
        {
            return Ok(None);
        }
        if !matches!(model.source.as_str(), "huggingface" | "ollama") {
            return Ok(None);
        }
        let Some(projector) = models::resolve_projector(
            &self.download_client,
            &model.source,
            &model.reference,
            &model.file,
        )
        .await?
        else {
            return Ok(None);
        };
        let profile = super::catalog::MachineProfile::detect(self.ctx.paths.base());
        Ok(projector.size.filter(|size| {
            projector.sha256.as_deref().is_some_and(valid_sha256)
                && super::vision::limits_for(&profile, model.size_bytes, *size).is_some()
        }))
    }

    /// Add just the vision weights to an already installed AI.
    pub async fn enable_vision(self: &Arc<Self>) -> Result<()> {
        let _install = self.install_lock.lock().await;
        let model = self.active_model().ok_or_else(|| anyhow!("no AI model"))?;
        if model.projector.is_some() {
            return Ok(());
        }
        let ticket = download::DownloadTicket {
            kind: "model",
            id: format!("vision:{}", model.reference),
            name: model.name.clone(),
        };
        let control = download::DownloadControl::new();
        crate::core::mutex_lock(&self.downloads).insert(ticket.id.clone(), control.clone());
        let previous = snapshot_previous(self).expect("active model");
        *crate::core::mutex_lock(&self.vision_error) = None;
        let mut downloaded: Option<String> = None;
        let mut started = false;
        let result = async {
            let resolved = self
                .eligible_projector(
                    &model.source,
                    &model.reference,
                    &model.file,
                    model.size_bytes,
                )
                .await?
                .ok_or_else(|| anyhow!("image-unavailable"))?;
            let events: Arc<dyn crate::core::Events> = Arc::new(InstallEvents {
                events: self.ctx.events.clone(),
                offset: 0,
                total: resolved.size,
            });
            let projector = self
                .download_projector(&resolved, &ticket, &control, &events)
                .await?;
            downloaded = Some(projector.file.clone());
            self.ctx.events.emit("rebost://download", json!({ "kind": ticket.kind, "id": ticket.id, "name": ticket.name, "phase": "preparing", "done": false }));
            self.wait_until_chat_idle().await;
            control.check_cancelled()?;
            started = true;
            self.stop().await;
            if let Some(active) = crate::core::write_lock(&self.ctx.settings)
                .active_model
                .as_mut()
            {
                active.projector = Some(projector);
            }
            self.ctx.save_settings();
            *crate::core::mutex_lock(&self.disabled_vision) = None;
            if let Err(error) = self.ensure_ready().await {
                log::error!("engine start with image support failed: {error:#}");
                return Err(anyhow!("image-start-failed"));
            }
            if self.vision_limits().is_none() {
                return Err(anyhow!("image-start-failed"));
            }
            Ok(())
        }
        .await;
        if result.is_err()
            && self
                .active_model()
                .is_some_and(|active| active.projector.is_some())
        {
            self.stop().await;
            restore_previous(self, previous);
            if let Err(error) = self.ensure_ready().await {
                log::error!("could not restore text AI after vision upgrade: {error:#}");
            }
        }
        if let (Err(error), Some(file)) = (&result, downloaded.as_deref()) {
            // The previous AI had no image file, so nothing else uses this one.
            let _ = std::fs::remove_file(self.ctx.paths.models_dir().join(file));
            if started {
                log::error!(
                    "image support for {} did not start ({error}); {}",
                    model.file,
                    self.vision_error()
                        .unwrap_or_else(|| "no reason in the engine log".into())
                );
                crate::core::write_lock(&self.ctx.settings).vision_failed_for =
                    Some(super::vision::failed_vision_key(&model.file));
                self.ctx.save_settings();
            }
        }
        crate::core::mutex_lock(&self.downloads).remove(&ticket.id);
        self.ctx.events.emit("rebost://download", json!({ "kind": ticket.kind, "id": ticket.id, "name": ticket.name, "done": true, "error": result.as_ref().err().map(|error| error.to_string()) }));
        result
    }

    async fn wait_until_chat_idle(&self) {
        // A long answer can run well past a few seconds. Switching AI here
        // would kill llama-server under the stream.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30 * 60);
        loop {
            if self.generation_in_flight.load(Ordering::Relaxed) == 0 {
                tokio::time::sleep(Duration::from_millis(200)).await;
                if self.generation_in_flight.load(Ordering::Relaxed) == 0 {
                    return;
                }
            }
            if tokio::time::Instant::now() >= deadline {
                log::warn!("timed out waiting for Chat to finish before switching AI");
                return;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn snapshot_previous(engine: &Engine) -> Option<PreviousAi> {
    let settings = crate::core::read_lock(&engine.ctx.settings);
    Some(PreviousAi {
        model: settings.active_model.clone()?,
        benchmark: settings.benchmark.clone(),
        context_budget_chars: settings.context_budget_chars,
    })
}

fn restore_previous(engine: &Engine, previous: PreviousAi) {
    {
        let mut settings = crate::core::write_lock(&engine.ctx.settings);
        settings.active_model = Some(previous.model);
        settings.benchmark = previous.benchmark;
        settings.context_budget_chars = previous.context_budget_chars;
    }
    engine.ctx.save_settings();
}

/// First-install warmup failed: keep the file, forget it as the active AI
/// so the next launch does not retry a load that already died.
fn clear_failed_first_install(engine: &Engine) {
    {
        let mut settings = crate::core::write_lock(&engine.ctx.settings);
        settings.active_model = None;
        settings.benchmark = None;
        settings.context_budget_chars = None;
    }
    engine.ctx.save_settings();
    engine.set_status(EngineState::NoModel, None);
}

/// If the live AI already uses this file name, write beside it so the
/// running process keeps the old bytes until the new one is Ready.
fn install_file_name(requested: &str, live_file: Option<&str>) -> String {
    if live_file == Some(requested) {
        sibling_install_name(requested)
    } else {
        requested.to_string()
    }
}

fn sibling_install_name(file_name: &str) -> String {
    let stem = file_name
        .strip_suffix(".gguf")
        .or_else(|| file_name.strip_suffix(".GGUF"))
        .unwrap_or(file_name);
    format!("{stem}.next.gguf")
}

fn previous_file_to_remove(previous: Option<&str>, new_file: &str) -> Option<String> {
    previous
        .filter(|file| *file != new_file)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_file_name_writes_beside_the_live_file() {
        assert_eq!(
            install_file_name("gemma.gguf", Some("gemma.gguf")),
            "gemma.next.gguf"
        );
        assert_eq!(
            install_file_name("gemma.gguf", Some("other.gguf")),
            "gemma.gguf"
        );
        assert_eq!(install_file_name("gemma.gguf", None), "gemma.gguf");
    }

    #[test]
    fn previous_file_stays_until_the_new_name_differs() {
        assert_eq!(
            previous_file_to_remove(Some("old.gguf"), "new.gguf").as_deref(),
            Some("old.gguf")
        );
        assert_eq!(
            previous_file_to_remove(Some("same.gguf"), "same.gguf"),
            None
        );
        assert_eq!(previous_file_to_remove(None, "new.gguf"), None);
    }
}
