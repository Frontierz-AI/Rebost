# Models

The installed catalog is `src-tauri/src/engine/catalog.rs` (`CATALOG`, `recommend`, alternatives). The app never calls Artificial Analysis.

## Recommendation

`recommend()` picks the first **document-work** catalog row that fits (the catalog is ordered by capability, highest first):

`runtime = file_bytes * 1.15 + 2 GiB`, fits when `runtime ≤ RAM * 0.55`.

The catalog is general document and office models (mixed-language shelves, chat, recipes). Coding checkpoints and single-language specialists are not listed; Explore can still find them.

Explore other models searches Hugging Face and Ollama and merges results. Hugging Face hits show public download counts, put the original publisher first (Qwen's own GGUFs before community quants), then sort by `createdAt` newest first. Settings also shows up to two catalog suggestions that fit and are not the installed model (`uninstalled_suggestions`, same order as first run). Install resolves a single-file GGUF, requires a SHA-256, then replaces the previous weights. After download, Skip check and proceed uses the file without hashing.

## Licenses

The app is MIT. Weights are not. The UI shows the upstream license before download. Gemma has its own terms; Apache-2.0/MIT models are still not covered by Rebost's MIT grant. See [licensing.md](licensing.md).

## Changing the catalog

1. Edit `CATALOG` in `catalog.rs`.
2. Adjust `recommend` / `smaller_alternatives` if the policy changes.
3. Run `cargo test --manifest-path src-tauri/Cargo.toml catalog` (and `model_catalog` ignored tests if you touch live APIs).
