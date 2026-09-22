# Models

The installed catalog is `src-tauri/src/engine/catalog.rs` (`CATALOG`, `recommend`, alternatives).

## Recommendation

`recommend()` picks the first **document-work** catalog row that fits (the catalog is ordered by capability, highest first):

`runtime = file_bytes * 1.15 + 2 GiB`, fits when `runtime ≤ RAM * 0.65`.

Capability order is set on the maintainer machine, never at runtime. Family heads use `CatalogStanding` in `catalog.rs`:

- **Scored** — Artificial Analysis Intelligence Index, higher first.
- **BenchLead** — no Index yet. Published benches show a clear lead over the current document pick for the RAM bands the new row would take. The row sorts just above that pick’s score (`BenchLead { above }`). Ornith-1.5 9B is the current example (`above: 14`, ahead of Gemma 4 12B).

A coding-only sweep does not unseat a document default. Overlapping benches the incumbent also reports count (Terminal-Bench v2.1, HLE, GPQA Diamond, and office or document benches when both sides publish them). Clear means ahead on at least two of those, and not clearly behind on the rest.

The catalog is general document and office models (mixed-language shelves, chat, recipes). Coding checkpoints and single-language specialists are not listed; Explore can still find them.

Explore other AIs is a Settings modal. An empty search browses popular Hugging Face GGUFs tagged `text-generation` or general `image-text-to-text`; a typed query also merges Ollama. A pasted `owner/repo` or Hugging Face model page URL looks that repo up directly and keeps it first, even when browse would hide a specialist. Default order mixes fit, recency (last 90 days), how much of the machine's memory the file uses, Official, then download counts. The list can be re-sorted by release date, download size, or download count, and **See more** pages 50 at a time. Hugging Face hits show public download counts and mark **Official** only for original-lab Hub namespaces (`ORIGINAL_MAKERS` in `models.rs`: Qwen, google, meta-llama, and similar, never quantizers or forks). Ollama library hits are Ollama's packaging, so they are never Official. Browse keeps general chat AIs (`text-generation`, or vision-chat without OCR/layout tags). Repos that only ship a projector stack and are not general chat are hidden, as are Hub specialists (OCR, layout, coder). A search that includes `ocr`, `coder`, or `code` can surface those specialists. Experimental `custom_*` / packed 1-bit files and CI stubs such as `ggml-org/models-moved` stay hidden. Settings also shows up to two catalog suggestions that fit and are not the installed model (`uninstalled_suggestions`, same order as first run). Those alternatives are other families: the strongest step-down (a smaller download, not a near-twin of the suggested AI), then the remaining row whose file is closest to the suggested AI. Install resolves a single-file GGUF, requires a SHA-256, then replaces the previous weights. After download, Skip the check and use the file uses the file without hashing.

## Licenses

The app is MIT. Weights are not. The UI shows the upstream license before download. Gemma has its own terms; Apache-2.0/MIT models are still not covered by Rebost's MIT grant. See [licensing.md](licensing.md).

## Changing the catalog

1. Edit `CATALOG` in `catalog.rs`. Set `standing` to `Scored` or `BenchLead`.
2. Keep family heads in `CatalogStanding::sort_key` descending.
3. Adjust `recommend` / `smaller_alternatives` if the policy changes.
4. Run `cargo test --manifest-path src-tauri/Cargo.toml catalog` (and `model_catalog` ignored tests if live APIs are touched).

## Image-capable models

Install also resolves an unambiguous matching `mmproj` GGUF from the same Hugging Face repository, or the projector layer in an Ollama manifest. The complete package must fit the machine budget, and the companion must have a SHA-256. Ambiguous repositories remain text-only. Existing installations can add only their missing companion from Settings. Downloads share progress and cancellation; the old model remains installed until the replacement is ready.

Vision requires at least 8 GiB RAM and room for the language weights, projector, ordinary runtime overhead, and another 512 MiB within the existing 65% model budget. Conservative ceilings are one image at 768px on CPU/small machines, two at 1024px on accelerated machines with at least 16 GiB, and four at 1536px on Metal with at least 32 GiB and 4 GiB remaining headroom. Context capacity can lower these counts or disable vision. Discrete GPU memory is not measured, so their image encoder runs in system RAM and keeps the middle tier ceiling. These are resource guardrails, not promises of model accuracy.

The runtime starts with `--mmproj` and `--image-max-tokens`, then must confirm vision through `/props`. A vision startup failure retries text-only. A compute failure disables vision for that model for the session. Pending images are never silently converted into a text-only question. Image embeddings are reserved separately when fitting the context because `/tokenize` counts only their text markers.

For a real local integration check, run `cargo test --test vision_smoke -- --ignored --nocapture` from `src-tauri` with `REBOST_ENGINE_ARCHIVE`, `REBOST_VISION_MODEL`, and `REBOST_VISION_PROJECTOR` pointing to compatible local files. The test uses an isolated data directory. Set `REBOST_VISION_REPO` to the matching Hugging Face repository to also check discovery and upgrading an existing text-only installation using the cached companion.
