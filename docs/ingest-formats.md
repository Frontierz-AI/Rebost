# Adding an ingest format

1. Confirm [xberg](https://crates.io/crates/xberg) already understands the format, or add the crate feature in `src-tauri/Cargo.toml`.
2. Extend `REBOST_EXTENSIONS` / `is_supported_file` in `src-tauri/src/ingest/extract.rs`.
3. Add a small fixture under `src-tauri/tests/fixtures/` and a case in `pipeline_test.rs` (and retrieval eval if the format should be searchable).
4. Mention the extension in Diagnostics (`supported_extensions`).

Unsupported files are skipped. They should not appear as errors on the Shelf.

## Web pages

HTML goes through Xberg's HTML extractor with Rebost's own conversion options (`html_options` in `extract.rs`): the Aggressive cleanup preset, images and frames left out, and a short list of cookie and consent banner selectors. The resulting Markdown has its YAML front matter and link targets removed before it is cached or indexed. A page inside a `Name_files/` folder next to `Name.html` (a browser's "Save page as, complete") is skipped by `is_saved_page_asset` in `shelf/scan.rs`, which looks at the disk, so the folder walk, the watcher, and `process_file` use `is_shelf_document` rather than `is_supported_file` alone.
