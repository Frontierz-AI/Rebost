//! Conversation search for the sidebar: message hits grouped by conversation,
//! each with a short plain-text snippet around the first match.
//!
//! Stemming only helps messages whose language `guess_message_lang` detected
//! when they were indexed (at least three words, two of them stopwords of one
//! language). Short messages match on folded exact words only.

use anyhow::Result;
use regex::Regex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::LazyLock;

use crate::chat::conversations::Conversations;
use crate::core::Ctx;
use crate::search::SearchIndex;
use crate::types::MemorySnippet;

pub const DEFAULT_LIMIT: usize = 50;
pub const MAX_LIMIT: usize = 200;
/// Longer input adds nothing: the query keeps at most 24 content words.
const MAX_QUERY_CHARS: usize = 512;
const SNIPPET_CHARS: usize = 120;
/// Context kept before the first match.
const LEAD_CHARS: usize = 40;

/// Snippet text, split where highlighting starts and stops. Render as text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetSegment {
    pub text: String,
    pub mark: bool,
}

/// One conversation whose messages match.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSearchHit {
    pub thread_id: String,
    /// Best-scoring message in this conversation.
    pub message_id: String,
    pub role: String,
    pub snippet: Vec<SnippetSegment>,
    pub created_at: String,
    /// Matching messages in this conversation, within the search window.
    pub matches: u32,
    /// The search window filled up, so `matches` may be higher in reality.
    pub matches_capped: bool,
}

/// Search saved messages and return one hit per live conversation, best first.
pub fn search(ctx: &Ctx, query: &str, limit: usize) -> Result<Vec<ThreadSearchHit>> {
    let query: String = query.chars().take(MAX_QUERY_CHARS).collect();
    let limit = limit.clamp(1, MAX_LIMIT);
    let window = limit * 4;
    let hits = ctx
        .search
        .search_messages(&query, None, None, &[], window)?;
    if hits.is_empty() {
        return Ok(Vec::new());
    }
    let capped = hits.len() >= window;
    // One read of threads.json; `Conversations::get` would re-read it per hit.
    let live: HashSet<String> = Conversations::list(&ctx.paths)
        .into_iter()
        .map(|thread| thread.id)
        .collect();
    Ok(group_hits(hits, &live, limit)
        .into_iter()
        .map(|(best, matches)| ThreadSearchHit {
            snippet: snippet(&ctx.search, &best.body, &query),
            thread_id: best.thread_id,
            message_id: best.message_id,
            role: best.role,
            created_at: best.created_at,
            matches,
            matches_capped: capped,
        })
        .collect())
}

/// Keep the first (best-scoring) hit per live thread and count the rest.
fn group_hits(
    hits: Vec<MemorySnippet>,
    live: &HashSet<String>,
    limit: usize,
) -> Vec<(MemorySnippet, u32)> {
    let mut sorted = hits;
    sorted.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut grouped: Vec<(MemorySnippet, u32)> = Vec::new();
    let mut slots: HashMap<String, usize> = HashMap::new();
    for hit in sorted {
        if !live.contains(&hit.thread_id) {
            continue;
        }
        if let Some(&slot) = slots.get(&hit.thread_id) {
            grouped[slot].1 += 1;
        } else {
            slots.insert(hit.thread_id.clone(), grouped.len());
            grouped.push((hit, 1));
        }
    }
    grouped.truncate(limit);
    grouped
}

/// About 120 characters of plain text around the first match, with matches marked.
pub fn snippet(search: &SearchIndex, body: &str, query: &str) -> Vec<SnippetSegment> {
    // The language guess runs on the stored text, as it did when indexing.
    let lang = crate::chat::guess_message_lang(body);
    let plain = plain_text(body);
    let ranges = search.highlight_ranges(&plain, query, lang);
    clip(&plain, &ranges, SNIPPET_CHARS)
}

static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s{0,3}(?:```|~~~)").unwrap());
static RULE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s{0,3}(?:(?:-\s*){3,}|(?:\*\s*){3,}|(?:_\s*){3,}|\|?[\s:|-]*-{3,}[\s:|-]*)$")
        .unwrap()
});
static LINE_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s{0,3}(?:#{1,6}\s+|>\s?|[-*+]\s+(?:\[[ xX]\]\s+)?|\d{1,9}[.)]\s+)").unwrap()
});
static CITATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*\[S\d+(?:\s*,\s*S\d+)*\]").unwrap());
static IMAGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!\[([^\]]*)\]\([^)]*\)").unwrap());
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\([^)]*\)").unwrap());
static HTML_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"</?[A-Za-z][^>]*>").unwrap());
static EMPHASIS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*+|~~|`+|\|").unwrap());
static UNDERSCORE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|[^\p{L}\p{N}])_+|_+([^\p{L}\p{N}]|$)").unwrap());

/// Message Markdown as one line of plain text: no syntax, citation markers,
/// links or tags. Code keeps its text without the fences.
pub fn plain_text(markdown: &str) -> String {
    let mut lines = Vec::new();
    for line in markdown.lines() {
        if FENCE.is_match(line) || RULE.is_match(line) {
            continue;
        }
        let mut line = line;
        // Nested prefixes such as "> - item".
        for _ in 0..3 {
            match LINE_PREFIX.find(line) {
                Some(prefix) if prefix.end() > 0 => line = &line[prefix.end()..],
                _ => break,
            }
        }
        lines.push(line);
    }
    let text = lines.join("\n");
    let text = CITATION.replace_all(&text, "");
    let text = IMAGE.replace_all(&text, "$1");
    let text = LINK.replace_all(&text, "$1");
    let text = HTML_TAG.replace_all(&text, " ");
    let text = EMPHASIS.replace_all(&text, " ");
    let text = UNDERSCORE.replace_all(&text, "$1$2");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Cut `text` to about `max_chars` characters around the first range,
/// on character boundaries, preferring word edges.
fn clip(text: &str, ranges: &[Range<usize>], max_chars: usize) -> Vec<SnippetSegment> {
    let boundaries: Vec<usize> = text
        .char_indices()
        .map(|(at, _)| at)
        .chain(std::iter::once(text.len()))
        .collect();
    let total = boundaries.len() - 1;
    let (mut start, mut end) = (0, text.len());
    if total > max_chars {
        let first = ranges.first().map_or(0, |range| range.start);
        let first_char = boundaries.partition_point(|&at| at < first);
        let start_char = first_char.saturating_sub(LEAD_CHARS).min(total - max_chars);
        start = boundaries[start_char];
        end = boundaries[start_char + max_chars];
        if start > 0 {
            // Begin at the next word rather than mid-word, unless that skips the match.
            if let Some(space) = text[start..first.max(start)].find(char::is_whitespace) {
                start += space;
            }
        }
        if end < text.len() {
            let keep = ranges.first().map_or(start, |range| range.end.min(end));
            if let Some(space) = text[keep..end].rfind(char::is_whitespace) {
                end = keep + space;
            }
        }
    }
    let mut out: Vec<SnippetSegment> = Vec::new();
    if start > 0 {
        push(&mut out, "…", false);
    }
    let mut cursor = start;
    for range in ranges {
        if range.end <= start || range.start >= end {
            continue;
        }
        let (from, to) = (range.start.max(cursor), range.end.min(end));
        if from >= to {
            continue;
        }
        push(&mut out, &text[cursor..from], false);
        push(&mut out, &text[from..to], true);
        cursor = to;
    }
    push(&mut out, &text[cursor..end], false);
    if end < text.len() {
        push(&mut out, "…", false);
    }
    if let Some(first) = out.first_mut().filter(|segment| !segment.mark) {
        first.text = first.text.trim_start().to_string();
        if first.text == "…" {
            first.text = "… ".to_string();
        }
    }
    out.retain(|segment| !segment.text.is_empty());
    out
}

fn push(out: &mut Vec<SnippetSegment>, text: &str, mark: bool) {
    if text.is_empty() {
        return;
    }
    match out.last_mut() {
        Some(last) if !last.mark && !mark => last.text.push_str(text),
        _ => out.push(SnippetSegment {
            text: text.to_string(),
            mark,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(segments: &[SnippetSegment]) -> String {
        segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect()
    }

    fn marks(segments: &[SnippetSegment]) -> Vec<&str> {
        segments
            .iter()
            .filter(|segment| segment.mark)
            .map(|segment| segment.text.as_str())
            .collect()
    }

    fn hit(thread: &str, message: &str, score: f32) -> MemorySnippet {
        MemorySnippet {
            thread_id: thread.into(),
            message_id: message.into(),
            role: "user".into(),
            body: String::new(),
            created_at: String::new(),
            score,
        }
    }

    #[test]
    fn plain_text_drops_markdown_and_citations() {
        let body = "## Notice period\n\n- The **notice** is _90 days_ [S1].\n> See [the contract](https://example.com) [S2, S3]\n\n```rust\nlet snake_case = 1;\n```\n| a | b |\n|---|---|\n| <b>x</b> | `y` |";
        assert_eq!(
            plain_text(body),
            "Notice period The notice is 90 days. See the contract let snake_case = 1; a b x y"
        );
    }

    #[test]
    fn short_text_is_kept_whole() {
        let segments = clip("Hola món", std::slice::from_ref(&(5..9)), 120);
        assert_eq!(joined(&segments), "Hola món");
        assert_eq!(marks(&segments), ["món"]);
    }

    #[test]
    fn clipping_respects_accents_and_emoji() {
        let text = format!(
            "{} pressupost 🎉 àéíòú {}",
            "café ñandú 😀 ".repeat(12),
            "😀 ò ".repeat(40)
        );
        let at = text.find("pressupost").unwrap();
        let segments = clip(
            &text,
            std::slice::from_ref(&(at..at + "pressupost".len())),
            120,
        );
        let out = joined(&segments);
        assert!(out.starts_with('…'));
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= 124, "{out}");
        assert_eq!(marks(&segments), ["pressupost"]);
        let before = out.split("pressupost").next().unwrap();
        assert!(before.chars().count() <= LEAD_CHARS + 2, "{before}");
    }

    #[test]
    fn clipping_without_a_match_starts_at_the_beginning() {
        let text = "ñ".repeat(300);
        let segments = clip(&text, &[], 120);
        let out = joined(&segments);
        assert!(out.starts_with('ñ'));
        assert_eq!(out.chars().count(), 121);
        assert!(marks(&segments).is_empty());
    }

    #[test]
    fn grouping_keeps_the_best_hit_and_counts_matches() {
        let live: HashSet<String> = ["t_a", "t_b"].map(String::from).into();
        let hits = vec![
            hit("t_a", "m_1", 1.0),
            hit("t_b", "m_2", 3.0),
            hit("t_a", "m_3", 2.0),
            hit("t_gone", "m_4", 9.0),
            hit("t_a", "m_5", 0.5),
        ];
        let grouped = group_hits(hits, &live, 50);
        let summary: Vec<(&str, &str, u32)> = grouped
            .iter()
            .map(|(best, count)| (best.thread_id.as_str(), best.message_id.as_str(), *count))
            .collect();
        assert_eq!(summary, [("t_b", "m_2", 1), ("t_a", "m_3", 3)]);
        assert_eq!(group_hits(vec![hit("t_a", "m", 1.0)], &live, 1).len(), 1);
    }
}
