//! Extra search queries for Light and Deep: one short pass before retrieval.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::engine::{ChatMessage, Engine};

pub(crate) async fn extra_search_queries(
    engine: &Arc<Engine>,
    question: &str,
    count: usize,
    cancel: &Arc<AtomicBool>,
) -> Vec<String> {
    if count == 0 || cancel.load(Ordering::Relaxed) {
        return Vec::new();
    }
    let messages = vec![
        ChatMessage::text(
            "system",
            "You write search queries that find answers in the user's files. Output only the queries.",
        ),
        ChatMessage::text("user", extra_query_prompt(question, count)),
    ];
    match engine.chat_once(&messages, 0.3, 200, cancel).await {
        Ok(output) => {
            if cancel.load(Ordering::Relaxed) {
                return Vec::new();
            }
            let mut parsed = parse_search_queries(&output.answer, question, count);
            if parsed.is_empty() && !output.thinking.is_empty() {
                parsed = parse_search_queries(&output.thinking, question, count);
            }
            parsed
        }
        Err(error) => {
            log::warn!("extra search queries failed: {error:#}");
            Vec::new()
        }
    }
}

fn extra_query_prompt(question: &str, count: usize) -> String {
    format!(
        "Write exactly {count} search queries for finding the answer in the user's files.\n\
Same language as the question.\n\
Line 1: names, dates, amounts, and clause-like terms from the question. Keywords only.\n\
Line 2: a paraphrase that uses different wording.\n\
Line 3: a likely filename, heading, or section title.\n\
One query per line. No numbering, quotes, or commentary.\n\
Do not repeat the original question.\n\n\
Question:\n{question}"
    )
}

pub(crate) fn parse_search_queries(raw: &str, original: &str, count: usize) -> Vec<String> {
    let original_fold = folded_query(original);
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in raw.lines() {
        let cleaned = clean_query_line(line);
        if cleaned.chars().count() < 3 {
            continue;
        }
        let fold = folded_query(&cleaned);
        if fold == original_fold || !seen.insert(fold) {
            continue;
        }
        out.push(cleaned);
        if out.len() >= count {
            break;
        }
    }
    out
}

fn clean_query_line(line: &str) -> String {
    let mut text = line.trim();
    if let Some(stripped) = text.strip_prefix("```") {
        text = stripped.trim();
    }
    text = text.trim_start_matches(['-', '*', '•']).trim();
    if let Some((index, rest)) = text.split_once(['.', ')', ':']) {
        if !index.is_empty() && index.chars().all(|c| c.is_ascii_digit()) {
            text = rest.trim();
        }
    }
    text.trim_matches(|c| c == '"' || c == '\'' || c == '`')
        .trim()
        .to_string()
}

fn folded_query(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_strips_numbering_and_skips_the_original() {
        let raw = "1. bad leaver permanence period\n\
- termination of executive director\n\
3. Which penalties do I have if I leave the company as CEO in the first years?\n\
non-compete after leaving\n";
        let original =
            "Which penalties do I have if I leave the company as CEO in the first years?";
        let got = parse_search_queries(raw, original, 3);
        assert_eq!(
            got,
            vec![
                "bad leaver permanence period",
                "termination of executive director",
                "non-compete after leaving",
            ]
        );
    }

    #[test]
    fn parse_dedupes_and_caps() {
        let raw = "zebra east wing\nZebra East Wing\nkitchen restock\nmore\n";
        let got = parse_search_queries(raw, "When is the kitchen restocked?", 2);
        assert_eq!(got, vec!["zebra east wing", "kitchen restock"]);
    }

    #[test]
    fn three_query_prompt_asks_for_distinct_jobs() {
        let prompt = extra_query_prompt("When can they terminate?", 3);
        assert!(prompt.contains("Line 1:"));
        assert!(prompt.contains("Line 2:"));
        assert!(prompt.contains("Line 3:"));
        assert!(prompt.contains("filename"));
    }
}
