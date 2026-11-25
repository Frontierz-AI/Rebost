//! Where a conversation search query occurs in a message, for snippets.

use std::collections::HashSet;
use std::ops::Range;
use tantivy::tokenizer::TokenStream;

use super::query::is_search_stopword;
use super::schema::STEM_LANGS;
use super::SearchIndex;

impl SearchIndex {
    /// Byte ranges in `text` that match `query` the way message search does:
    /// the same folded exact tokens (case and accents ignored, stopwords
    /// skipped) and, when the message has a stem language, the same stems.
    /// Sorted, without overlaps. Empty when the query has no content words.
    pub fn highlight_ranges(
        &self,
        text: &str,
        query: &str,
        lang: Option<&str>,
    ) -> Vec<Range<usize>> {
        let exact: HashSet<String> = self.content_tokens(query).into_iter().collect();
        if exact.is_empty() {
            return Vec::new();
        }
        let mut ranges = self.matching_ranges("rebost_exact", text, &exact);
        if let Some(lang) = lang.filter(|lang| STEM_LANGS.contains(lang)) {
            let joined = exact.iter().cloned().collect::<Vec<_>>().join(" ");
            let stems: HashSet<String> = self
                .stem_tokens(lang, &joined)
                .into_iter()
                .filter(|stem| !is_search_stopword(stem))
                .collect();
            if !stems.is_empty() {
                ranges.extend(self.matching_ranges(&format!("rebost_stem_{lang}"), text, &stems));
            }
        }
        ranges.sort_by_key(|range| (range.start, range.end));
        let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
        for range in ranges {
            match merged.last_mut() {
                Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
                _ => merged.push(range),
            }
        }
        merged
    }

    fn matching_ranges(
        &self,
        analyzer: &str,
        text: &str,
        wanted: &HashSet<String>,
    ) -> Vec<Range<usize>> {
        let Some(mut analyzer) = self.index.tokenizers().get(analyzer) else {
            return Vec::new();
        };
        let mut ranges = Vec::new();
        let mut stream = analyzer.token_stream(text);
        while let Some(token) = stream.next() {
            if wanted.contains(&token.text) {
                ranges.push(token.offset_from..token.offset_to);
            }
        }
        ranges
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> (tempfile::TempDir, SearchIndex) {
        let dir = tempfile::tempdir().unwrap();
        let search = SearchIndex::open(dir.path()).unwrap();
        (dir, search)
    }

    fn marked<'a>(text: &'a str, ranges: &[Range<usize>]) -> Vec<&'a str> {
        ranges.iter().map(|range| &text[range.clone()]).collect()
    }

    #[test]
    fn matches_ignore_case_and_accents() {
        let (_dir, search) = index();
        let text = "La FACTURA d'Àlex i la factura de l'Alex";
        let ranges = search.highlight_ranges(text, "alex factura", None);
        assert_eq!(
            marked(text, &ranges),
            ["FACTURA", "Àlex", "factura", "Alex"]
        );
    }

    #[test]
    fn stems_match_in_the_message_language() {
        let (_dir, search) = index();
        let text = "Quan comença la vacança d'estiu?";
        let ranges = search.highlight_ranges(text, "vacances", Some("ca"));
        assert_eq!(marked(text, &ranges), ["vacança"]);
        assert!(search.highlight_ranges(text, "vacances", None).is_empty());
    }

    #[test]
    fn stopword_only_queries_mark_nothing() {
        let (_dir, search) = index();
        assert!(search
            .highlight_ranges("the cat and the dog", "the and", Some("en"))
            .is_empty());
    }
}
