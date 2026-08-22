# Models

The installed catalog is `src-tauri/src/engine/catalog.rs` (`CATALOG`, `recommend`, alternatives). The app never calls Artificial Analysis.

## Recommendation

`recommend()` picks the first **document-work** catalog row that fits (the catalog is ordered by capability, highest first):

`runtime = file_bytes * 1.15 + 2 GiB`, fits when `runtime ≤ RAM * 0.55`.

The catalog is general document and office models (mixed-language shelves, chat, recipes). Coding checkpoints and single-language specialists are not listed; Explore can still find them.

Explore other AIs is a Settings modal. An empty search browses popular Hugging Face GGUFs tagged `text-generation` or general `image-text-to-text`; a typed query also merges Ollama. A pasted `owner/repo` or Hugging Face model page URL looks that repo up directly and keeps it first, even when browse would hide a specialist. Default order mixes fit, recency (last 90 days), how much of the machine's memory the file uses, Official, then download counts. The list can be re-sorted by release date, download size, or download count, and **See more** pages 50 at a time. Hugging Face hits show public download counts and mark **Official** only for original-lab Hub namespaces (`ORIGINAL_MAKERS` in `models.rs`: Qwen, google, meta-llama, and similar — never quantizers or forks). Ollama library hits are Ollama's packaging, so they are never Official. Browse keeps general chat AIs (`text-generation`, or vision-chat without OCR/layout tags). Repos that only ship a projector stack and are not general chat are hidden, as are Hub specialists (OCR, layout, coder). A search that includes `ocr`, `coder`, or `code` can surface those specialists. Experimental `custom_*` / packed 1-bit files and CI stubs such as `ggml-org/models-moved` stay hidden. Settings also shows up to two catalog suggestions that fit and are not the installed model (`uninstalled_suggestions`, same order as first run). Install resolves a single-file GGUF, requires a SHA-256, then replaces the previous weights. After download, Skip the check and use the file uses the file without hashing.

## Licenses

The app is MIT. Weights are not. The UI shows the upstream license before download. Gemma has its own terms; Apache-2.0/MIT models are still not covered by Rebost's MIT grant. See [licensing.md](licensing.md).

## Changing the catalog

1. Edit `CATALOG` in `catalog.rs`.
2. Adjust `recommend` / `smaller_alternatives` if the policy changes.
3. Run `cargo test --manifest-path src-tauri/Cargo.toml catalog` (and `model_catalog` ignored tests if live APIs are touched).
