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
    /// With `last_word_as_prefix`, words that start with the query's last
    /// word count too, as in [`search_messages_as_typed`](Self::search_messages_as_typed).
    pub fn highlight_ranges(
        &self,
        text: &str,
        query: &str,
        lang: Option<&str>,
        last_word_as_prefix: bool,
    ) -> Vec<Range<usize>> {
        let exact: HashSet<String> = self.content_tokens(query).into_iter().collect();
        if exact.is_empty() {
            return Vec::new();
        }
        let prefix = last_word_as_prefix
            .then(|| self.typed_prefix(query))
            .flatten();
        let mut ranges = self.matching_ranges("rebost_exact", text, &exact, prefix.as_deref());
        if let Some(lang) = lang.filter(|lang| STEM_LANGS.contains(lang)) {
            let joined = exact.iter().cloned().collect::<Vec<_>>().join(" ");
            let stems: HashSet<String> = self
                .stem_tokens(lang, &joined)
                .into_iter()
                .filter(|stem| !is_search_stopword(stem))
                .collect();
            if !stems.is_empty() {
                ranges.extend(self.matching_ranges(
                    &format!("rebost_stem_{lang}"),
                    text,
                    &stems,
                    None,
                ));
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
        prefix: Option<&str>,
    ) -> Vec<Range<usize>> {
        let Some(mut analyzer) = self.index.tokenizers().get(analyzer) else {
            return Vec::new();
        };
        let mut ranges = Vec::new();
        let mut stream = analyzer.token_stream(text);
        while let Some(token) = stream.next() {
            let starts = prefix.is_some_and(|prefix| token.text.starts_with(prefix));
            if starts || wanted.contains(&token.text) {
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
        let ranges = search.highlight_ranges(text, "alex factura", None, false);
        assert_eq!(
            marked(text, &ranges),
            ["FACTURA", "Àlex", "factura", "Alex"]
        );
    }

    #[test]
    fn stems_match_in_the_message_language() {
        let (_dir, search) = index();
        let text = "Quan comença la vacança d'estiu?";
        let ranges = search.highlight_ranges(text, "vacances", Some("ca"), false);
        assert_eq!(marked(text, &ranges), ["vacança"]);
        assert!(search
            .highlight_ranges(text, "vacances", None, false)
            .is_empty());
    }

    #[test]
    fn an_unfinished_last_word_marks_words_that_start_with_it() {
        let (_dir, search) = index();
        let text = "Tomatoes and a TOMATO salad, not a tomb";
        let typed = search.highlight_ranges(text, "salad tomat", None, true);
        assert_eq!(marked(text, &typed), ["Tomatoes", "TOMATO", "salad"]);
        // Only the last word is unfinished ("sal" does not mark "salad"),
        // and only in typed mode.
        assert_eq!(
            marked(
                text,
                &search.highlight_ranges(text, "sal tomato", None, true)
            ),
            ["Tomatoes", "TOMATO"]
        );
        assert!(search
            .highlight_ranges(text, "tomat", None, false)
            .is_empty());
        // Two letters are too few to search as a prefix.
        assert!(search.highlight_ranges(text, "to", None, true).is_empty());
    }

    #[test]
    fn stopword_only_queries_mark_nothing() {
        let (_dir, search) = index();
        assert!(search
            .highlight_ranges("the cat and the dog", "the and", Some("en"), false)
            .is_empty());
    }
}
