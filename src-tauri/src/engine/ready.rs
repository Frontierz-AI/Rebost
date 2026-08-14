//! Spawn llama-server and wait until `/health` succeeds.

use anyhow::{anyhow, Context, Result};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

use super::process::{
    engine_log_tail, force_kill_pid, free_port, kill_stale_llama_servers, pipe_to_log,
};
use super::{Engine, EngineState, Inner, ENGINE_BUILD};
use crate::settings::ActiveModel;

const HEALTH_TIMEOUT: Duration = Duration::from_secs(240);
const CONTEXT_TOKENS: u32 = 8192;

impl Engine {
    fn model_path(&self, model: &ActiveModel) -> std::path::PathBuf {
        self.ctx.paths.models_dir().join(&model.file)
    }

    async fn health_ok(&self, port: u16) -> bool {
        let url = format!("http://127.0.0.1:{port}/health");
        matches!(
            self.client
                .get(&url)
                .timeout(Duration::from_secs(2))
                .send()
                .await,
            Ok(response) if response.status().is_success()
        )
    }

    /// Bring the engine up if Chat needs it. Queued callers simply await.
    /// Returns the base URL for requests.
    pub async fn ensure_ready(self: &std::sync::Arc<Self>) -> Result<String> {
        let Some(model) = self.active_model() else {
            self.set_status(EngineState::NoModel, None);
            return Err(anyhow!("no AI model installed yet"));
        };
        let model_path = self.model_path(&model);
        if !model_path.exists() {
            self.set_status(
                EngineState::NoModel,
                Some("The AI model file is missing.".into()),
            );
            return Err(anyhow!("model file missing"));
        }

        {
            let mut inner = self.inner.lock().await;
            if let Some(url) = self.live_url(&mut inner, &model).await {
                return Ok(url);
            }
        }

        let _start = self.start_lock.lock().await;
        {
            let mut inner = self.inner.lock().await;
            if let Some(url) = self.live_url(&mut inner, &model).await {
                return Ok(url);
            }
        }

        // Download llama.cpp without holding the process lock — otherwise the
        // first chat sits on "Warming up…" with no engine log for minutes.
        let binary = self.ensure_binary().await?;
        let data_dir = self.ctx.paths.base().to_path_buf();
        tokio::task::spawn_blocking(move || kill_stale_llama_servers(&data_dir))
            .await
            .ok();

        let mut inner = self.inner.lock().await;
        if let Some(url) = self.live_url(&mut inner, &model).await {
            return Ok(url);
        }

        let port = free_port()?;
        log::info!("starting llama-server {} with {}", ENGINE_BUILD, model.file);
        self.set_status(EngineState::Starting, None);

        let log_path = self.ctx.paths.logs_dir().join("engine.log");
        std::fs::create_dir_all(self.ctx.paths.logs_dir()).ok();

        let mut command = Command::new(&binary);
        if let Some(home) = binary.parent() {
            command.current_dir(home);
        }
        command
            .args([
                "-m",
                model_path.to_string_lossy().as_ref(),
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "-c",
                &CONTEXT_TOKENS.to_string(),
                // One conversation at a time. Auto slot count (4) times 8k
                // context blew GPU memory on a 30B model, especially if a
                // previous llama-server was still holding the last load.
                "-np",
                "1",
                "-b",
                "512",
                "-ngl",
                "99",
                "--jinja",
                // Extract template-declared reasoning into reasoning_content
                // (DeepSeek-R1, Qwen3, …); inline-tag models are split
                // client-side, and the system prompt asks untagged reasoners
                // to tag their thinking.
                "--reasoning-format",
                "auto",
                "--no-webui",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        {
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command.spawn().context("spawn llama-server")?;

        if let Some(stdout) = child.stdout.take() {
            let log_path = log_path.clone();
            tokio::spawn(pipe_to_log(stdout, log_path));
        }
        if let Some(stderr) = child.stderr.take() {
            let log_path = log_path.clone();
            tokio::spawn(pipe_to_log(stderr, log_path));
        }

        let started = std::time::Instant::now();
        let mut last_note = started;
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                let tail = engine_log_tail(&log_path);
                log::error!("llama-server exited early ({status}); log tail:\n{tail}");
                self.set_status(
                    EngineState::Error,
                    Some("The AI model couldn't start. Try again.".into()),
                );
                return Err(anyhow!("llama-server exited early ({status})"));
            }
            if self.health_ok(port).await {
                break;
            }
            if started.elapsed() > HEALTH_TIMEOUT {
                child.kill().await.ok();
                let tail = engine_log_tail(&log_path);
                log::error!(
                    "llama-server health timeout after {}s; log tail:\n{tail}",
                    started.elapsed().as_secs()
                );
                self.set_status(
                    EngineState::Error,
                    Some("The AI model took too long to start. Try again.".into()),
                );
                return Err(anyhow!("llama-server health timeout"));
            }
            if last_note.elapsed() >= Duration::from_secs(10) {
                let secs = started.elapsed().as_secs();
                log::info!("still loading {} ({}s)", model.name, secs);
                last_note = std::time::Instant::now();
            }
            tokio::time::sleep(Duration::from_millis(400)).await;
        }

        inner.child = Some(child);
        inner.port = port;
        inner.model_file = model.file.clone();
        drop(inner);

        log::info!(
            "llama-server ready on 127.0.0.1:{port} after {}s",
            started.elapsed().as_secs()
        );
        self.set_status(EngineState::Ready, None);

        let engine = self.clone();
        let base_url = format!("http://127.0.0.1:{port}");
        let bench_base = base_url.clone();
        tokio::spawn(async move {
            super::bench::run_if_needed(&engine, &bench_base).await;
        });

        Ok(base_url)
    }

    async fn live_url(&self, inner: &mut Inner, model: &ActiveModel) -> Option<String> {
        let child = inner.child.as_mut()?;
        let exited = child.try_wait().ok().flatten().is_some();
        let port = inner.port;
        let same_model = inner.model_file == model.file;
        if !exited && same_model && self.health_ok(port).await {
            self.set_status(EngineState::Ready, None);
            Some(format!("http://127.0.0.1:{port}"))
        } else {
            if let Some(mut child) = inner.child.take() {
                child.kill().await.ok();
            }
            None
        }
    }

    /// Closing Rebost stops the engine and releases its memory.
    pub async fn stop(&self) {
        let mut inner = self.inner.lock().await;
        if let Some(mut child) = inner.child.take() {
            child.kill().await.ok();
        }
        drop(inner);
        let data_dir = self.ctx.paths.base().to_path_buf();
        tokio::task::spawn_blocking(move || kill_stale_llama_servers(&data_dir))
            .await
            .ok();
        let has_model = self.active_model().is_some();
        self.set_status(
            if has_model {
                EngineState::Stopped
            } else {
                EngineState::NoModel
            },
            None,
        );
    }

    pub fn stop_blocking(&self) {
        if let Ok(mut inner) = self.inner.try_lock() {
            if let Some(child) = inner.child.as_mut() {
                if let Some(pid) = child.id() {
                    force_kill_pid(pid);
                }
            }
            inner.child = None;
        }
        kill_stale_llama_servers(self.ctx.paths.base());
    }
}
