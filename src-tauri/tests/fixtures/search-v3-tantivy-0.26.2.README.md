# Search v3 migration fixture

`search-v3-tantivy-0.26.2.tar.gz` is a real, committed Tantivy 0.26.2 index, generated from Rebost's `search/schema.rs` and `search/stems.rs` at commit `86078dd998d2c593226f5f3458751d82970bf08c`. Its schema marker is `rebost-search/v3`.

It contains one synthetic user message: thread `legacy-fixture-thread`, message `legacy-fixture-message`, text `legacy-index-only-sentinel migration fixture`, timestamp 1785571200. It contains no user data. The two temporary Tantivy lock files are excluded; archive timestamps, owner IDs and permissions are normalized.

Generation used a temporary Rust crate with `tantivy = "=0.26.2"` and `tantivy-stemmers = "=0.4.0"` with the same twelve language features as Rebost. Copy the two source modules from the commit above, call `build_schema()` and `register_tokenizers()`, create the index with `Index::create_in_dir`, add the message through an `IndexWriter`, commit and call `wait_merging_threads()`. Add the v3 schema marker before archiving.

`index_migration.rs` installs these original index bytes alongside synthetic durable document caches and conversation JSONL. It verifies that the v4 migration restores disconnected-source documents and valid chat messages, preserves durable bytes, retries an incomplete restoration and avoids duplicate results on reopen. Corrupt inventories keep restoration pending; valid metadata backups and empty first-run state are also covered.
