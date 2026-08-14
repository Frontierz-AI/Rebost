//! Chat SSE consumption: thinking vs answer, cancellation, compute retry.

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::process::is_compute_failure;
use super::think::{SplitOut, ThinkSplitter};
use super::{ChatOutput, Engine, StreamEvent};

impl Engine {
    /// Stream a chat completion. Reasoning traces (from `reasoning_content`
    /// deltas or inline `<think>` blocks) are separated from the answer and
    /// both are streamed through `on_event`. Returns the collected output.
    pub async fn chat_stream(
        self: &Arc<Self>,
        messages: &[super::ChatMessage],
        temperature: f32,
        max_tokens: u32,
        cancel: &Arc<AtomicBool>,
        mut on_event: impl FnMut(StreamEvent<'_>),
    ) -> Result<ChatOutput> {
        let body = json!({
            "messages": messages,
            "stream": true,
            "temperature": temperature,
            "max_tokens": max_tokens,
            "cache_prompt": true,
            "chat_template_kwargs": {"enable_thinking": false},
        });
        let mut retried = false;
        loop {
            let base = self.ensure_ready().await?;
            let response = self
                .client
                .post(format!("{base}/v1/chat/completions"))
                .json(&body)
                .send()
                .await
                .context("chat request")?;
            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                if !retried && is_compute_failure(&text) {
                    retried = true;
                    self.recover_after_compute_failure(&text).await;
                    continue;
                }
                return Err(anyhow!("generation failed ({status}): {text}"));
            }

            let stream = response.bytes_stream();
            match consume_sse(stream, cancel, &mut on_event).await {
                Ok(mut output) => {
                    output.answer = output.answer.trim().to_string();
                    output.thinking = output.thinking.trim().to_string();
                    return Ok(output);
                }
                Err(error) => {
                    let message = error.to_string();
                    if !retried && is_compute_failure(&message) {
                        retried = true;
                        self.recover_after_compute_failure(&message).await;
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }

    async fn recover_after_compute_failure(&self, detail: &str) {
        log::error!("engine compute failed; restarting: {detail}");
        self.stop().await;
    }

    /// One-shot completion returning llama.cpp's timing block — used only by
    /// the installation benchmark. Takes the base URL directly so it never
    /// re-enters `ensure_ready` (which spawns the benchmark).
    pub async fn completion_timings(
        self: &Arc<Self>,
        base: &str,
        prompt: &str,
    ) -> Result<super::Timings> {
        let response = self
            .client
            .post(format!("{base}/completion"))
            .json(&json!({
                "prompt": prompt,
                "n_predict": 24,
                "cache_prompt": false,
            }))
            .send()
            .await?
            .error_for_status()?;
        let value: serde_json::Value = response.json().await?;
        let timings = &value["timings"];
        Ok(super::Timings {
            prompt_per_second: timings["prompt_per_second"].as_f64().unwrap_or(0.0),
            predicted_per_second: timings["predicted_per_second"].as_f64().unwrap_or(0.0),
        })
    }
}

/// Read an OpenAI-style SSE chat stream until `[DONE]`, cancel, or error.
pub(crate) async fn consume_sse<S, E>(
    mut stream: S,
    cancel: &Arc<AtomicBool>,
    on_event: &mut impl FnMut(StreamEvent<'_>),
) -> Result<ChatOutput>
where
    S: futures_util::Stream<Item = Result<bytes::Bytes, E>> + Unpin,
    E: std::fmt::Display + Send + Sync + 'static,
{
    let mut output = ChatOutput::default();
    let mut splitter = ThinkSplitter::new();
    let mut buffer = String::new();

    let apply = |events: Vec<SplitOut>,
                 output: &mut ChatOutput,
                 on_event: &mut dyn FnMut(StreamEvent<'_>)| {
        for event in events {
            output.apply(&event);
            match &event {
                SplitOut::Thinking(t) => on_event(StreamEvent::Thinking(t)),
                SplitOut::Answer(t) => on_event(StreamEvent::Answer(t)),
                SplitOut::Promote => on_event(StreamEvent::PromoteAnswerToThinking),
            }
        }
    };

    let mut stream_error: Option<String> = None;
    'outer: while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let chunk = chunk.map_err(|error| anyhow!("chat stream: {error}"))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(line_end) = buffer.find('\n') {
            let line = buffer[..line_end].trim().to_string();
            buffer.drain(..=line_end);
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            if data == "[DONE]" {
                break 'outer;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };
            if let Some(error) = value["error"]["message"].as_str() {
                stream_error = Some(error.to_string());
                break 'outer;
            }
            if let Some(error) = value["error"].as_str() {
                stream_error = Some(error.to_string());
                break 'outer;
            }
            let delta = &value["choices"][0]["delta"];
            if let Some(piece) = delta["reasoning_content"].as_str() {
                if !piece.is_empty() {
                    output.thinking.push_str(piece);
                    on_event(StreamEvent::Thinking(piece));
                }
            }
            if let Some(piece) = delta["content"].as_str() {
                apply(splitter.push(piece), &mut output, on_event);
            }
        }
    }
    if let Some(error) = stream_error {
        return Err(anyhow!("generation failed: {error}"));
    }
    apply(splitter.flush(), &mut output, on_event);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures_util::stream;
    use std::sync::Mutex;

    fn delta(text: &str) -> Bytes {
        Bytes::from(format!(
            "data: {{\"choices\":[{{\"delta\":{{\"content\":{}}}}}]}}\n\n",
            serde_json::to_string(text).unwrap()
        ))
    }

    #[tokio::test]
    async fn cancel_stops_sse_mid_stream() {
        let cancel = Arc::new(AtomicBool::new(false));
        let stream = stream::iter([
            Ok::<_, std::io::Error>(delta("Hello there, this is the visible answer.")),
            Ok(delta(" SECRET")),
        ]);
        let collected = Mutex::new(String::new());
        let output = consume_sse(stream, &cancel, &mut |event| {
            if let StreamEvent::Answer(t) = event {
                crate::core::mutex_lock(&collected).push_str(t);
                cancel.store(true, Ordering::Relaxed);
            }
        })
        .await
        .unwrap();
        assert!(
            output.answer.starts_with("Hello there"),
            "{:?}",
            output.answer
        );
        assert!(
            !output.answer.contains("SECRET"),
            "cancelled stream still included {:?}",
            output.answer
        );
    }

    #[tokio::test]
    async fn sse_error_object_is_a_failure() {
        let cancel = Arc::new(AtomicBool::new(false));
        let stream = stream::iter([Ok::<_, std::io::Error>(Bytes::from(
            "data: {\"error\":{\"message\":\"compute error\"}}\n\n",
        ))]);
        let err = consume_sse(stream, &cancel, &mut |_| {}).await.unwrap_err();
        assert!(err.to_string().contains("compute error"), "{err:#}");
    }
}
