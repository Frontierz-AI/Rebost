//! Surround a search hit with the text before and after it.

use std::collections::HashMap;

use crate::core::Ctx;
use crate::types::SourcePassage;

pub(crate) const OFF_RADIUS_CHARS: usize = 400;
pub(crate) const LIGHT_RADIUS_CHARS: usize = 700;
pub(crate) const DEEP_RADIUS_CHARS: usize = 1_000;
pub(crate) const LOOK_AROUND_RADIUS_CHARS: usize = 1_600;
const SNAP_CHARS: usize = 80;

pub(crate) fn widen_hit_body(extracted: &str, body: &str, radius: usize) -> String {
    let body = body.trim();
    if body.is_empty() || extracted.is_empty() {
        return body.to_string();
    }
    let Some(pos) = extracted.find(body) else {
        return body.to_string();
    };
    let body_end = pos + body.len();
    let start = snap_start(extracted, pos.saturating_sub(radius), pos);
    let end = snap_end(
        extracted,
        body_end.saturating_add(radius).min(extracted.len()),
    );
    extracted[start..end].trim().to_string()
}

fn floor_char(s: &str, mut i: usize) -> usize {
    if i > s.len() {
        i = s.len();
    }
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn snap_start(s: &str, approx: usize, hit: usize) -> usize {
    let i = floor_char(s, approx.min(hit));
    if i == 0 {
        return 0;
    }
    let limit = (i + SNAP_CHARS).min(hit);
    let slice = &s[i..limit];
    if let Some(nl) = slice.find('\n') {
        floor_char(s, i + nl + 1)
    } else {
        i
    }
}

fn snap_end(s: &str, approx: usize) -> usize {
    let i = floor_char(s, approx.min(s.len()));
    if i >= s.len() {
        return s.len();
    }
    let limit = (i + SNAP_CHARS).min(s.len());
    let slice = &s[i..limit];
    if let Some(nl) = slice.find('\n') {
        floor_char(s, i + nl)
    } else {
        i
    }
}

pub(crate) fn widen_neighbor_passages(
    ctx: &Ctx,
    shelf_id: &str,
    mut passages: Vec<SourcePassage>,
    radius: usize,
) -> Vec<SourcePassage> {
    let mut extracted: HashMap<String, String> = HashMap::new();
    for passage in &mut passages {
        if passage.body.is_empty() {
            continue;
        }
        let text = extracted
            .entry(passage.document_id.clone())
            .or_insert_with(|| {
                std::fs::read_to_string(ctx.paths.extracted_path(shelf_id, &passage.document_id))
                    .unwrap_or_default()
            });
        if text.is_empty() {
            continue;
        }
        let wider = widen_hit_body(text, &passage.body, radius);
        if wider.len() > passage.body.len() {
            passage.body = wider;
        }
    }
    passages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widen_includes_text_before_and_after_the_hit() {
        let extracted =
            "alpha paragraph.\n\nThe kitchen is restocked on Tuesday.\n\nomega paragraph.";
        let body = "The kitchen is restocked on Tuesday.";
        let out = widen_hit_body(extracted, body, 80);
        assert!(out.contains("alpha"));
        assert!(out.contains("Tuesday"));
        assert!(out.contains("omega"));
    }

    #[test]
    fn widen_returns_the_body_when_it_is_not_in_the_file() {
        let out = widen_hit_body("unrelated file", "missing passage", 80);
        assert_eq!(out, "missing passage");
    }
}
