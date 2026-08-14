//! Ranking queries over the passage and conversation-memory indexes.

use anyhow::Result;
use tantivy::collector::TopDocs;
use tantivy::query::{
    BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, PhraseQuery, Query, TermQuery,
};
use tantivy::schema::{Field, IndexRecordOption, Value};
use tantivy::tokenizer::TokenStream;
use tantivy::{TantivyDocument, Term};

use super::schema::{weights, STEM_LANGS};
use super::SearchIndex;
use crate::types::{MemorySnippet, SourcePassage};

impl SearchIndex {
    /// Tokens of `text` under the exact analyzer (lowercased, folded).
    fn exact_tokens(&self, text: &str) -> Vec<String> {
        let mut analyzer = self
            .index
            .tokenizers()
            .get("rebost_exact")
            .expect("rebost_exact registered");
        let mut tokens = Vec::new();
        let mut stream = analyzer.token_stream(text);
        while let Some(token) = stream.next() {
            tokens.push(token.text.clone());
            if tokens.len() >= 24 {
                break;
            }
        }
        tokens
    }

    fn stem_tokens(&self, lang: &str, text: &str) -> Vec<String> {
        let Some(mut analyzer) = self.index.tokenizers().get(&format!("rebost_stem_{lang}")) else {
            return Vec::new();
        };
        let mut tokens = Vec::new();
        let mut stream = analyzer.token_stream(text);
        while let Some(token) = stream.next() {
            tokens.push(token.text.clone());
            if tokens.len() >= 24 {
                break;
            }
        }
        tokens
    }

    /// Build the passage query: exact terms across weighted fields, stemmed
    /// terms across every language field (documents fill exactly one),
    /// a phrase bonus, and cautious fuzzy matching for typos — never on
    /// numeric or short tokens, so business identifiers stay precise.
    fn passage_query(&self, message: &str, shelf_id: &str) -> Box<dyn Query> {
        let tokens = self.exact_tokens(message);
        let mut should: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        let weighted_fields: [(Field, f32); 5] = [
            (self.fields.title, weights::TITLE),
            (self.fields.keywords, weights::KEYWORDS),
            (self.fields.section, weights::SECTION),
            (self.fields.body, weights::BODY),
            (self.fields.summary, weights::SUMMARY),
        ];

        for token in &tokens {
            for (field, weight) in weighted_fields {
                let term = Term::from_field_text(field, token);
                let tq: Box<dyn Query> =
                    Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs));
                should.push((Occur::Should, Box::new(BoostQuery::new(tq, weight))));
            }
            // Fuzzy for reasonable typos on alphabetic tokens only.
            let alphabetic = token.chars().all(|c| c.is_alphabetic());
            if alphabetic && token.chars().count() >= 5 {
                let distance = if token.chars().count() >= 9 { 2 } else { 1 };
                for field in [self.fields.body, self.fields.title] {
                    let term = Term::from_field_text(field, token);
                    let fq: Box<dyn Query> = Box::new(FuzzyTermQuery::new(term, distance, true));
                    should.push((Occur::Should, Box::new(BoostQuery::new(fq, weights::FUZZY))));
                }
            }
        }

        // Stemmed terms — one field per language; a document only carries its
        // own language field, so cross-language noise stays out.
        for lang in STEM_LANGS {
            let stem_field = self.fields.stems[*lang];
            for token in self.stem_tokens(lang, message) {
                let term = Term::from_field_text(stem_field, &token);
                let tq: Box<dyn Query> =
                    Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs));
                should.push((
                    Occur::Should,
                    Box::new(BoostQuery::new(tq, weights::STEMMED)),
                ));
            }
        }

        // Phrase bonus keeps multi-word references precise.
        if tokens.len() >= 2 && tokens.len() <= 12 {
            let terms: Vec<Term> = tokens
                .iter()
                .map(|t| Term::from_field_text(self.fields.body, t))
                .collect();
            let pq: Box<dyn Query> = Box::new(PhraseQuery::new(terms));
            should.push((
                Occur::Should,
                Box::new(BoostQuery::new(pq, weights::PHRASE)),
            ));
        }

        let relevance: Box<dyn Query> = Box::new(BooleanQuery::new(should));
        let mut clauses: Vec<(Occur, Box<dyn Query>)> = vec![
            (
                Occur::Must,
                Box::new(TermQuery::new(
                    Term::from_field_text(self.fields.record_type, "passage"),
                    IndexRecordOption::Basic,
                )),
            ),
            (Occur::Must, relevance),
        ];
        clauses.push((
            Occur::Must,
            Box::new(TermQuery::new(
                Term::from_field_text(self.fields.shelf_id, shelf_id),
                IndexRecordOption::Basic,
            )),
        ));
        Box::new(BooleanQuery::new(clauses))
    }

    /// Search Shelf passages for the user message, unchanged.
    pub fn search_passages(
        &self,
        message: &str,
        shelf_id: &str,
        limit: usize,
    ) -> Result<Vec<SourcePassage>> {
        if self.exact_tokens(message).is_empty() {
            return Ok(Vec::new());
        }
        let searcher = self.reader.searcher();
        let query = self.passage_query(message, shelf_id);
        let top = searcher.search(&query, &TopDocs::with_limit(limit).order_by_score())?;
        let mut hits = Vec::new();
        for (score, address) in top {
            let doc: TantivyDocument = searcher.doc(address)?;
            let get_str = |f: Field| -> String {
                doc.get_first(f)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string()
            };
            let get_u64 = |f: Field| -> Option<u32> {
                doc.get_first(f).and_then(|v| v.as_u64()).map(|v| v as u32)
            };
            hits.push(SourcePassage {
                sid: String::new(),
                document_id: get_str(self.fields.document_id),
                shelf_id: get_str(self.fields.shelf_id),
                title: get_str(self.fields.title),
                section: {
                    let s = get_str(self.fields.section);
                    if s.is_empty() {
                        None
                    } else {
                        Some(s)
                    }
                },
                page_start: get_u64(self.fields.page_start),
                page_end: get_u64(self.fields.page_end),
                body: get_str(self.fields.body),
                path: get_str(self.fields.path),
                score,
            });
        }
        Ok(hits)
    }

    /// Search older conversation memory, excluding the active thread.
    pub fn search_messages(
        &self,
        message: &str,
        exclude_thread: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemorySnippet>> {
        let tokens = self.exact_tokens(message);
        if tokens.is_empty() {
            return Ok(Vec::new());
        }
        let mut should: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for token in &tokens {
            let term = Term::from_field_text(self.fields.body, token);
            should.push((
                Occur::Should,
                Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs)),
            ));
        }
        for lang in STEM_LANGS {
            let stem_field = self.fields.stems[*lang];
            for token in self.stem_tokens(lang, message) {
                let term = Term::from_field_text(stem_field, &token);
                let tq: Box<dyn Query> =
                    Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs));
                should.push((Occur::Should, Box::new(BoostQuery::new(tq, 0.9))));
            }
        }
        let relevance: Box<dyn Query> = Box::new(BooleanQuery::new(should));
        let mut clauses: Vec<(Occur, Box<dyn Query>)> = vec![
            (
                Occur::Must,
                Box::new(TermQuery::new(
                    Term::from_field_text(self.fields.record_type, "message"),
                    IndexRecordOption::Basic,
                )),
            ),
            (Occur::Must, relevance),
        ];
        if let Some(thread) = exclude_thread {
            clauses.push((
                Occur::MustNot,
                Box::new(TermQuery::new(
                    Term::from_field_text(self.fields.thread_id, thread),
                    IndexRecordOption::Basic,
                )),
            ));
        }
        let query = BooleanQuery::new(clauses);
        let searcher = self.reader.searcher();
        let top = searcher.search(&query, &TopDocs::with_limit(limit).order_by_score())?;
        let mut hits = Vec::new();
        for (score, address) in top {
            let doc: TantivyDocument = searcher.doc(address)?;
            let get_str = |f: Field| -> String {
                doc.get_first(f)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string()
            };
            let created = doc
                .get_first(self.fields.created_at)
                .and_then(|v| v.as_datetime())
                .map(|d| {
                    chrono::DateTime::<chrono::Utc>::from_timestamp(d.into_timestamp_secs(), 0)
                        .map(|d| d.to_rfc3339())
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            hits.push(MemorySnippet {
                thread_id: get_str(self.fields.thread_id),
                message_id: get_str(self.fields.message_id),
                role: get_str(self.fields.role),
                body: get_str(self.fields.body),
                created_at: created,
                score,
            });
        }
        Ok(hits)
    }

    /// Total records, for diagnostics.
    pub fn num_docs(&self) -> u64 {
        self.reader.searcher().num_docs()
    }

    /// Query tokens under the exact analyzer — used by the gate's overlap check.
    pub fn query_tokens(&self, text: &str) -> Vec<String> {
        self.exact_tokens(text)
    }
}

#[cfg(test)]
mod tests {
    use crate::search::SearchIndex;
    use crate::types::{DocStatus, DocumentMeta, Passage, SourceType};

    fn index() -> (tempfile::TempDir, SearchIndex) {
        let dir = tempfile::tempdir().unwrap();
        let search = SearchIndex::open(dir.path()).unwrap();
        (dir, search)
    }

    fn meta(id: &str, shelf: &str) -> DocumentMeta {
        DocumentMeta {
            id: id.into(),
            shelf_id: shelf.into(),
            source_id: "imported".into(),
            source_type: SourceType::Imported,
            path: format!("/{id}.md"),
            rel_path: format!("{id}.md"),
            file_name: format!("{id}.md"),
            format: "md".into(),
            size_bytes: 32,
            hash: format!("sha256:{id}"),
            status: DocStatus::Ready,
            error: None,
            passage_count: 1,
            pages: None,
            pii_total: 0,
            pii_categories: Default::default(),
            ocr: false,
            updated_at: "2026-01-01T00:00:00Z".into(),
            source_label: "Imported".into(),
        }
    }

    fn passage(body: &str) -> Passage {
        Passage {
            seq: 0,
            section: None,
            page_start: None,
            page_end: None,
            body: body.into(),
        }
    }

    fn add(search: &SearchIndex, id: &str, shelf: &str, title: &str, body: &str) {
        search
            .index_document(
                &meta(id, shelf),
                "",
                &[],
                &[passage(body)],
                Some("en"),
                "full",
                title,
            )
            .unwrap();
    }

    #[test]
    fn empty_or_punctuation_query_returns_nothing() {
        let (_dir, search) = index();
        add(&search, "d1", "s1", "Invoice", "INV-2026-0042 is due");
        assert!(search.search_passages("   ", "s1", 8).unwrap().is_empty());
        assert!(search.search_passages("...", "s1", 8).unwrap().is_empty());
    }

    #[test]
    fn exact_identifier_ranks_above_a_fuzzy_neighbour() {
        let (_dir, search) = index();
        add(
            &search,
            "exact",
            "s1",
            "Invoice INV-2026-0042",
            "Pay INV-2026-0042 by Friday.",
        );
        add(
            &search,
            "other",
            "s1",
            "Office move",
            "The painter booked Friday. Nothing about invoices here.",
        );
        let hits = search.search_passages("INV-2026-0042", "s1", 8).unwrap();
        assert!(!hits.is_empty());
        assert!(
            hits[0].title.contains("INV-2026-0042") || hits[0].body.contains("INV-2026-0042"),
            "exact business id should win, got {:?}",
            hits[0].title
        );
    }

    #[test]
    fn shelf_scoping_excludes_the_other_shelf() {
        let (_dir, search) = index();
        add(
            &search,
            "a",
            "shelf-a",
            "Secret A",
            "unique-alpha-token lives here",
        );
        add(
            &search,
            "b",
            "shelf-b",
            "Secret B",
            "unique-alpha-token lives here",
        );
        let a = search
            .search_passages("unique-alpha-token", "shelf-a", 8)
            .unwrap();
        let b = search
            .search_passages("unique-alpha-token", "shelf-b", 8)
            .unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].document_id, "a");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].document_id, "b");
        assert!(search
            .search_passages("unique-alpha-token", "shelf-missing", 8)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn short_tokens_do_not_fuzzy_match() {
        let (_dir, search) = index();
        add(&search, "d1", "s1", "Note", "The team meets on Monday.");
        // "term" is 4 letters — below the fuzzy threshold of 5.
        assert!(search.search_passages("term", "s1", 8).unwrap().is_empty());
    }

    #[test]
    fn longer_typos_still_reach_the_body() {
        let (_dir, search) = index();
        add(
            &search,
            "d1",
            "s1",
            "Agreement",
            "Either party may terminate this agreement with notice.",
        );
        let hits = search
            .search_passages("terminaton of the agreement", "s1", 8)
            .unwrap();
        assert!(!hits.is_empty(), "5+ letter typos should fuzzy-match");
    }
}
