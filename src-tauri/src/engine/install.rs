//! Model download + switch-over.

use anyhow::{anyhow, Result};
use serde_json::json;
use std::sync::Arc;

use super::download;
use super::models;
use super::{Engine, EngineState};

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

    /// Download and switch to a model. One model is installed at a time;
    /// the previous file is removed after a successful switch.
    pub async fn install_model(
        self: &Arc<Self>,
        source: &str,
        reference: &str,
        display_name: &str,
        license: Option<String>,
    ) -> Result<()> {
        let source = models::normalize_source(source)?;
        models::validate_reference(source, reference)?;

        let ticket = download::DownloadTicket {
            kind: "model",
            id: format!("model:{reference}"),
            name: display_name.to_string(),
        };
        let control = download::DownloadControl::new();
        crate::core::mutex_lock(&self.downloads).insert(ticket.id.clone(), control.clone());
        self.set_status(EngineState::Downloading, Some(display_name.to_string()));
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

        let resolved =
            match models::resolve_download(&self.download_client, source, reference).await {
                Ok(resolved) => resolved,
                Err(error) => {
                    crate::core::mutex_lock(&self.downloads).remove(&ticket.id);
                    let has_model = self.active_model().is_some();
                    self.set_status(
                        if has_model {
                            EngineState::Stopped
                        } else {
                            EngineState::NoModel
                        },
                        None,
                    );
                    return Err(error);
                }
            };
        let sha256 = resolved
            .sha256
            .as_deref()
            .ok_or_else(|| anyhow!("model file has no SHA-256 checksum; refusing to install"))?;
        let file_name = models::safe_model_file_name(&resolved.file_name)?;
        let dest = self.ctx.paths.models_dir().join(&file_name);

        let result = download::download(
            &self.download_client,
            &resolved.url,
            &dest,
            &ticket,
            Some(sha256),
            resolved.size,
            &self.ctx.events.clone(),
            &control,
        )
        .await;
        crate::core::mutex_lock(&self.downloads).remove(&ticket.id);
        if let Err(error) = result {
            let has_model = self.active_model().is_some();
            self.set_status(
                if has_model {
                    EngineState::Stopped
                } else {
                    EngineState::NoModel
                },
                None,
            );
            return Err(error);
        }

        let previous = self.active_model();
        {
            let mut settings = crate::core::write_lock(&self.ctx.settings);
            settings.active_model = Some(crate::settings::ActiveModel {
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
        self.stop().await;

        if let Some(previous) = previous {
            if previous.file != file_name {
                std::fs::remove_file(self.ctx.paths.models_dir().join(previous.file)).ok();
            }
        }

        let engine = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(error) = engine.ensure_ready().await {
                log::error!("engine warmup after install failed: {error:#}");
            }
        });
        Ok(())
    }
}
