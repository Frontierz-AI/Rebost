//! llama.cpp `llama-server` as a child process.
//!
//! Generation is its only job. The UI never sees a server, port, or PID;
//! it sees "Warming up..." and then tokens.

pub mod bench;
mod binary;
pub mod catalog;
pub mod download;
mod install;
pub mod models;
mod pin;
mod process;
mod ready;
mod stream;
mod think;

pub use pin::{
    current_engine_pin, find_bundled_engine_archive, pin_for_target_triple, EnginePin,
    ENGINE_BUILD, ENGINE_PINS,
};

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Child;

use crate::core::Ctx;
use crate::settings::ActiveModel;
use process::{kill_stale_llama_servers, USER_AGENT};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EngineState {
    NoModel,
    Downloading,
    Stopped,
    Starting,
    Ready,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub state: EngineState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
}

struct Inner {
    child: Option<Child>,
    port: u16,
    model_file: String,
}

pub struct Engine {
    ctx: Arc<Ctx>,
    pub client: reqwest::Client,
    download_client: reqwest::Client,
    inner: tokio::sync::Mutex<Inner>,
    /// Serializes first-time engine download + spawn so two chat/warmup
    /// callers cannot both fetch llama.cpp.
    start_lock: tokio::sync::Mutex<()>,
    status: std::sync::Mutex<EngineStatus>,
    downloads: std::sync::Mutex<HashMap<String, download::DownloadControl>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub struct Timings {
    pub prompt_per_second: f64,
    pub predicted_per_second: f64,
}

impl Engine {
    pub fn ctx(&self) -> &Arc<Ctx> {
        &self.ctx
    }

    pub fn catalog_client(&self) -> &reqwest::Client {
        &self.download_client
    }

    pub fn new(ctx: Arc<Ctx>) -> Arc<Self> {
        let model_name = crate::core::read_lock(&ctx.settings)
            .active_model
            .as_ref()
            .map(|m| m.name.clone());
        let state = if model_name.is_some() {
            EngineState::Stopped
        } else {
            EngineState::NoModel
        };
        kill_stale_llama_servers(ctx.paths.base());
        Arc::new(Self {
            ctx,
            client: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .connect_timeout(Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
            download_client: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .https_only(true)
                .redirect(reqwest::redirect::Policy::limited(5))
                .http1_only()
                .pool_max_idle_per_host(8)
                .connect_timeout(Duration::from_secs(30))
                .tcp_nodelay(true)
                .build()
                .expect("reqwest download client"),
            inner: tokio::sync::Mutex::new(Inner {
                child: None,
                port: 0,
                model_file: String::new(),
            }),
            start_lock: tokio::sync::Mutex::new(()),
            status: std::sync::Mutex::new(EngineStatus {
                state,
                detail: None,
                model_name,
            }),
            downloads: std::sync::Mutex::new(HashMap::new()),
        })
    }

    pub fn status(&self) -> EngineStatus {
        crate::core::mutex_lock(&self.status).clone()
    }

    fn set_status(&self, state: EngineState, detail: Option<String>) {
        let model_name = crate::core::read_lock(&self.ctx.settings)
            .active_model
            .as_ref()
            .map(|m| m.name.clone());
        let status = EngineStatus {
            state,
            detail,
            model_name,
        };
        *crate::core::mutex_lock(&self.status) = status.clone();
        if let Ok(payload) = serde_json::to_value(&status) {
            self.ctx.events.emit("rebost://engine", payload);
        }
    }

    fn active_model(&self) -> Option<ActiveModel> {
        crate::core::read_lock(&self.ctx.settings)
            .active_model
            .clone()
    }
}

/// A streamed piece of model output, already sorted into thinking vs answer.
pub enum StreamEvent<'a> {
    Thinking(&'a str),
    Answer(&'a str),
    /// The stream turned out to be reasoning-first without an opening tag
    /// (a lone `</think>` appeared): everything streamed as answer so far
    /// was actually thinking.
    PromoteAnswerToThinking,
}

#[derive(Debug, Default, Clone)]
pub struct ChatOutput {
    pub answer: String,
    pub thinking: String,
}
