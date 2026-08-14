//! Installation benchmark.
//!
//! Measures prompt processing on this machine and converts it into the
//! local-context budget Chat may spend per message. Users never see tokens
//! — internally: budget_chars ≈ prompt_tokens/s × 2s × ~3.6 chars/token,
//! clamped to sane bounds.

use super::Engine;
use std::sync::Arc;

const BENCH_TARGET_SECONDS: f64 = 2.0;
const CHARS_PER_TOKEN: f64 = 3.6;
const MIN_BUDGET: usize = 4_000;
const MAX_BUDGET: usize = 26_000;

fn bench_prompt() -> String {
    // ~700 tokens of plain business prose; content is irrelevant, size isn't.
    let paragraph = "The quarterly review covers revenue, invoices, payroll, supplier \
contracts, renewal dates, termination clauses, payment schedules, and the summary of \
open positions across departments. Each section lists the responsible owner, the due \
date, and the current state of the work in plain language. ";
    paragraph.repeat(18)
}

/// Run the benchmark once per installed model, storing the measured budget.
pub async fn run_if_needed(engine: &Arc<Engine>, base_url: &str) {
    let (model_file, already_measured) = {
        let settings = engine.ctx_settings();
        let model_file = match &settings.active_model {
            Some(model) => model.file.clone(),
            None => return,
        };
        let done = settings
            .benchmark
            .as_ref()
            .map(|b| b.model_file == model_file)
            .unwrap_or(false);
        (model_file, done)
    };
    if already_measured {
        return;
    }

    log::info!("running installation benchmark for {model_file}");
    let timings = match engine.completion_timings(base_url, &bench_prompt()).await {
        Ok(timings) => timings,
        Err(error) => {
            log::warn!("benchmark failed: {error:#}");
            if super::process::is_compute_failure(&error.to_string()) {
                engine.stop().await;
            }
            return;
        }
    };
    if timings.prompt_per_second <= 0.0 {
        return;
    }

    let budget = (timings.prompt_per_second * BENCH_TARGET_SECONDS * CHARS_PER_TOKEN) as usize;
    let budget = budget.clamp(MIN_BUDGET, MAX_BUDGET);
    log::info!(
        "benchmark: {:.0} prompt tok/s, {:.0} gen tok/s → context budget {} chars",
        timings.prompt_per_second,
        timings.predicted_per_second,
        budget
    );

    engine.store_benchmark(
        crate::settings::BenchmarkResult {
            prompt_tokens_per_second: timings.prompt_per_second,
            generation_tokens_per_second: timings.predicted_per_second,
            measured_at: chrono::Utc::now().to_rfc3339(),
            model_file,
        },
        budget,
    );
}

impl Engine {
    fn ctx_settings(&self) -> crate::settings::Settings {
        crate::core::read_lock(&self.ctx().settings).clone()
    }

    fn store_benchmark(&self, result: crate::settings::BenchmarkResult, budget: usize) {
        {
            let mut settings = crate::core::write_lock(&self.ctx().settings);
            settings.benchmark = Some(result);
            settings.context_budget_chars = Some(budget);
        }
        self.ctx().save_settings();
    }
}
