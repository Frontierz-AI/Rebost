# ADR 0003: Multilingual lexical stemming

- Status: accepted
- Date: 2026-08

Business documents inflect. The index keeps one stem field per supported language so "vacances" can match "vacança" (and the same for Spanish, Polish, and the rest) without a separate embedding model.

Languages in Tantivy's Snowball `Language` enum use the built-in `Stemmer`. Languages it does not ship use Snowball algorithms from `tantivy-stemmers`, wrapped in `search/stems.rs` because that crate's `TokenFilter` targets an older `tantivy-tokenizer-api` than tantivy 0.26.

Queries search every stem field. A document fills only the field for its detected language. Adding a language is a schema bump (`SCHEMA_VERSION`).

2026-10 update: Tantivy 0.27 uses `frostem` for its built-in stemmers. The adapter remains necessary for the twelve additional languages and the tokenizer API mismatch. The index moves to `rebost-search/v4` and rebuilds cached documents and conversation memory with the new tokenizers.
