//! The Retrieval Gate.
//!
//! Tantivy always returns *something*; the gate decides what is actually
//! relevant enough to hand to the model, and fits it into the machine's
//! measured prompt-processing budget. Deterministic and explainable:
//!
//! 1. relative score cut against the best hit,
//! 2. lexical-overlap check (at least one meaningful query token must
//!    literally appear in the passage — fuzzy-only matches don't clear),
//! 3. caps, then greedy budget fitting in score order.

use crate::types::{MemorySnippet, SourcePassage};

/// Gate constants — tuned against the retrieval eval suite.
pub mod tuning {
    /// Passages scoring below this fraction of the top hit are dropped.
    pub const DOC_RELATIVE_FLOOR: f32 = 0.30;
    /// Memory snippets are held to a stricter cut.
    pub const MSG_RELATIVE_FLOOR: f32 = 0.50;
    /// Hard cap on document passages sent to the model.
    pub const MAX_DOC_PASSAGES: usize = 8;
    /// Hard cap on older-conversation snippets.
    pub const MAX_MEMORY_SNIPPETS: usize = 3;
    /// Fraction of the local-context budget reserved for memory snippets.
    pub const MEMORY_BUDGET_SHARE: f32 = 0.18;
    /// Query tokens shorter than this don't count for the overlap check.
    pub const OVERLAP_MIN_TOKEN_LEN: usize = 3;
    /// Default local-context budget when no benchmark has run yet (chars).
    pub const DEFAULT_BUDGET_CHARS: usize = 9_000;
}

fn folded(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            _ => c,
        })
        .collect()
}

/// True when at least one meaningful query token appears literally in the
/// passage text (title, section or body), accent- and case-insensitively.
fn overlaps(query_tokens: &[String], haystacks: &[&str]) -> bool {
    let meaningful: Vec<&String> = query_tokens
        .iter()
        .filter(|t| t.chars().count() >= tuning::OVERLAP_MIN_TOKEN_LEN)
        .collect();
    if meaningful.is_empty() {
        // Nothing meaningful to check against — let the score cut decide.
        return true;
    }
    let folded_haystacks: Vec<String> = haystacks.iter().map(|h| folded(h)).collect();
    meaningful
        .iter()
        .any(|t| folded_haystacks.iter().any(|h| h.contains(t.as_str())))
}

/// Apply the gate to raw passage hits. Returns passages in score order with
/// their `[S1]`-style ids assigned.
pub fn gate_passages(mut hits: Vec<SourcePassage>, query_tokens: &[String]) -> Vec<SourcePassage> {
    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    let Some(top) = hits.first().map(|h| h.score) else {
        return Vec::new();
    };
    if top <= 0.0 {
        return Vec::new();
    }
    let mut kept: Vec<SourcePassage> = Vec::new();
    for hit in hits {
        if hit.score < top * tuning::DOC_RELATIVE_FLOOR {
            break;
        }
        let haystacks = [
            hit.title.as_str(),
            hit.section.as_deref().unwrap_or(""),
            hit.body.as_str(),
        ];
        if !overlaps(query_tokens, &haystacks) {
            continue;
        }
        kept.push(hit);
        if kept.len() >= tuning::MAX_DOC_PASSAGES {
            break;
        }
    }
    for (i, passage) in kept.iter_mut().enumerate() {
        passage.sid = format!("S{}", i + 1);
    }
    kept
}

/// Apply the gate to older-conversation hits.
pub fn gate_messages(mut hits: Vec<MemorySnippet>, query_tokens: &[String]) -> Vec<MemorySnippet> {
    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    let Some(top) = hits.first().map(|h| h.score) else {
        return Vec::new();
    };
    if top <= 0.0 {
        return Vec::new();
    }
    let mut kept = Vec::new();
    for hit in hits {
        if hit.score < top * tuning::MSG_RELATIVE_FLOOR {
            break;
        }
        if !overlaps(query_tokens, &[hit.body.as_str()]) {
            continue;
        }
        kept.push(hit);
        if kept.len() >= tuning::MAX_MEMORY_SNIPPETS {
            break;
        }
    }
    kept
}

/// Fit gated context into the machine's measured budget, favouring document
/// passages; memory gets a bounded share. Passages keep score order.
pub fn fit_to_budget(
    passages: Vec<SourcePassage>,
    memory: Vec<MemorySnippet>,
    budget_chars: usize,
) -> (Vec<SourcePassage>, Vec<MemorySnippet>) {
    let memory_budget = (budget_chars as f32 * tuning::MEMORY_BUDGET_SHARE) as usize;
    let doc_budget = budget_chars.saturating_sub(if memory.is_empty() { 0 } else { memory_budget });

    let mut kept_passages = Vec::new();
    let mut used = 0usize;
    for mut p in passages {
        let cost = p.body.chars().count() + p.title.chars().count() + 64;
        if used + cost > doc_budget {
            // Try a truncated version of the first over-budget passage so a
            // single long section can't starve the answer entirely.
            if kept_passages.is_empty() && doc_budget > 512 {
                let take = doc_budget.saturating_sub(p.title.chars().count() + 64);
                p.body = truncate_at_boundary(&p.body, take);
                kept_passages.push(p);
            }
            break;
        }
        used += cost;
        kept_passages.push(p);
    }
    // Re-assign sids after budget fitting so they stay dense.
    for (i, passage) in kept_passages.iter_mut().enumerate() {
        passage.sid = format!("S{}", i + 1);
    }

    let mut kept_memory = Vec::new();
    let mut used_memory = 0usize;
    for mut m in memory {
        let cost = m.body.chars().count() + 48;
        if used_memory + cost > memory_budget {
            if kept_memory.is_empty() && memory_budget > 256 {
                m.body = truncate_at_boundary(&m.body, memory_budget.saturating_sub(48));
                kept_memory.push(m);
            }
            break;
        }
        used_memory += cost;
        kept_memory.push(m);
    }

    (kept_passages, kept_memory)
}

/// Cut at a sentence or word boundary, never mid-word.
pub fn truncate_at_boundary(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let hard: String = text.chars().take(max_chars).collect();
    for boundary in ['\n', '.', ';'] {
        if let Some(pos) = hard.rfind(boundary) {
            if pos > max_chars / 2 {
                return format!("{}…", &hard[..=pos]);
            }
        }
    }
    match hard.rfind(' ') {
        Some(pos) if pos > max_chars / 2 => format!("{}…", &hard[..pos]),
        _ => format!("{hard}…"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passage(score: f32, body: &str) -> SourcePassage {
        SourcePassage {
            sid: String::new(),
            document_id: "d_x".into(),
            shelf_id: "s_x".into(),
            title: "Doc".into(),
            section: None,
            page_start: None,
            page_end: None,
            body: body.into(),
            path: "/tmp/doc.pdf".into(),
            score,
        }
    }

    #[test]
    fn empty_hits_stay_empty() {
        let out = gate_passages(Vec::new(), &["termination".into()]);
        assert!(out.is_empty());
    }

    #[test]
    fn relative_floor_cuts_weak_tail() {
        let hits = vec![
            passage(10.0, "termination clause applies"),
            passage(6.0, "termination notice period"),
            passage(1.0, "unrelated catering invoice"),
        ];
        let out = gate_passages(hits, &["termination".into()]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].sid, "S1");
        assert_eq!(out[1].sid, "S2");
    }

    #[test]
    fn overlap_check_drops_fuzzy_only_matches() {
        let hits = vec![
            passage(5.0, "termination clause applies"),
            // scored close but shares no literal token with the query
            passage(4.0, "wholly unrelated text"),
        ];
        let out = gate_passages(hits, &["termination".into()]);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn overlap_is_accent_insensitive() {
        let hits = vec![passage(5.0, "La rescisión del contrato se regula aquí")];
        let out = gate_passages(hits, &["rescision".into()]);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn budget_fitting_truncates_first_long_passage() {
        let long_body = "word ".repeat(4000);
        let hits = vec![passage(5.0, &long_body)];
        let (docs, _) = fit_to_budget(hits, Vec::new(), 2000);
        assert_eq!(docs.len(), 1);
        assert!(docs[0].body.chars().count() < 2000);
    }

    #[test]
    fn truncate_prefers_sentence_boundary() {
        let text = "First sentence. Second sentence that is much longer and rambles on.";
        let cut = truncate_at_boundary(text, 40);
        assert!(cut.starts_with("First sentence."));
        assert!(cut.ends_with('…'));
    }
}
