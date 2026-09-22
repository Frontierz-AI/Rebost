//! Check the rendered request with the loaded model's tokenizer before inference.

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

use super::Engine;

impl Engine {
    pub(super) async fn fit_request(
        &self,
        base: &str,
        body: &mut Value,
        flatten: bool,
    ) -> Result<()> {
        let Some(plan) = crate::core::read_lock(&self.ctx.runtime_plan).clone() else {
            return Ok(());
        };
        let image_count = count_images(body);
        let answer = body["max_tokens"]
            .as_u64()
            .unwrap_or(plan.answer_tokens as u64)
            .min(plan.answer_tokens as u64)
            .min(if image_count > 0 { 1024 } else { u64::MAX });
        body["max_tokens"] = json!(answer);
        let limit = (plan.context_tokens as usize).saturating_sub(answer as usize + 128);
        for _ in 0..24 {
            // Keep tool notes separate while trimming, even for templates that require inlining them.
            let mut request = body.clone();
            if flatten {
                let messages: Vec<super::ChatMessage> =
                    serde_json::from_value(body["messages"].clone())?;
                request["messages"] = json!(super::messages::flatten_tool_turns(&messages));
            }
            let rendered = self
                .client
                .post(format!("{base}/apply-template"))
                .timeout(Duration::from_secs(15))
                .json(&request)
                .send()
                .await?;
            if !rendered.status().is_success() {
                // Let the completion endpoint classify template/tool incompatibility.
                *body = request;
                return Ok(());
            }
            let rendered: Value = rendered.json().await?;
            let prompt = rendered["prompt"]
                .as_str()
                .context("missing rendered prompt")?;
            let tokens: Value = self
                .client
                .post(format!("{base}/tokenize"))
                .timeout(Duration::from_secs(15))
                .json(&json!({"content": prompt, "add_special": true}))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let count = tokens["tokens"]
                .as_array()
                .context("missing prompt tokens")?
                .len();
            // /tokenize only counts the media marker, not the image embeddings.
            let image_tokens = count_images(&request)
                * self
                    .vision_limits()
                    .map_or(2048, |limits| limits.tokens_per_image as usize + 256);
            if count.saturating_add(image_tokens) <= limit {
                *body = request;
                return Ok(());
            }
            if !reduce_context(body) {
                return Err(anyhow!("{}", rust_i18n::t!("errors.promptTooLong")));
            }
        }
        Err(anyhow!("{}", rust_i18n::t!("errors.promptTooLong")))
    }
}

fn count_images(body: &Value) -> usize {
    body["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["content"].as_array())
        .flatten()
        .filter(|part| part["type"] == "image_url")
        .count()
}

/// Never shorten the user's request or standing rules. Remove old dialogue
/// first, then shorten retrieved notes at a visible boundary.
fn reduce_context(body: &mut Value) -> bool {
    let Some(messages) = body["messages"].as_array_mut() else {
        return false;
    };
    let last_user = messages
        .iter()
        .rposition(|m| m["role"] == "user")
        .unwrap_or(0);
    if let Some(first) = (0..last_user).find(|i| messages[*i]["role"] != "system") {
        let end = (first + 1..=last_user)
            .find(|i| messages[*i]["role"] == "user")
            .unwrap_or(last_user);
        messages.drain(first..end);
        return true;
    }
    if let Some(message) = messages.iter_mut().rev().find(|m| {
        m["role"] == "tool"
            && m["content"]
                .as_str()
                .is_some_and(|s| s.chars().count() > 384)
    }) {
        let text = message["content"].as_str().unwrap_or_default();
        let chars = text.chars().count() * 2 / 3;
        message["content"] = json!(format!(
            "{}\n[Excerpt shortened to fit. Do not assume the omitted text.]",
            crate::search::gate::truncate_at_boundary(text, chars)
        ));
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn trimming_old_visual_turns_keeps_the_current_image() {
        let mut body = serde_json::json!({"messages":[
            {"role":"system","content":"rules"},
            {"role":"user","content":[{"type":"image_url","image_url":{"url":"old"}}]},
            {"role":"assistant","content":"old answer"},
            {"role":"user","content":[{"type":"text","text":"look"},{"type":"image_url","image_url":{"url":"current"}}]}
        ]});
        assert_eq!(super::count_images(&body), 2);
        assert!(super::reduce_context(&mut body));
        assert_eq!(super::count_images(&body), 1);
        assert_eq!(
            body["messages"][1]["content"][1]["image_url"]["url"],
            "current"
        );
        assert!(!super::reduce_context(&mut body));
    }

    use super::*;

    #[test]
    fn fitting_keeps_current_question_and_system_intact() {
        let mut body = json!({"messages":[
            {"role":"system","content":"rules"},
            {"role":"user","content":"old"},
            {"role":"assistant","content":"old answer"},
            {"role":"user","content":"current question"},
            {"role":"assistant","tool_calls":[]},
            {"role":"tool","content":"evidence ".repeat(200)}
        ]});
        assert!(reduce_context(&mut body));
        assert_eq!(body["messages"][1]["content"], "current question");
        while reduce_context(&mut body) {}
        assert_eq!(body["messages"][0]["content"], "rules");
        assert_eq!(body["messages"][1]["content"], "current question");
        assert!(body["messages"][3]["content"]
            .as_str()
            .unwrap()
            .contains("Excerpt shortened"));
    }
}
