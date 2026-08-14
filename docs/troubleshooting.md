# Troubleshooting

| Symptom | What to try |
|---------|-------------|
| Stuck on "Warming up..." | Settings → Diagnostics; read `engine.log`. Quit Rebost (kills leftover `llama-server`). Reset only if you can re-download the model. |
| Download sits at 100% or "Checking the download…" | SHA-256 of a large file can take a while. Skip check and proceed uses the file without hashing. A mismatch deletes the file; try again. Hugging Face 403: the resolve URL needs the `?download=true` path (already in code). |
| "The model catalogs couldn't be reached" | Network to huggingface.co / ollama.com. Search is optional; recommendation is local. |
| Install refused (no SHA-256) | That GGUF listing has no LFS oid / digest. Pick another file or catalog entry. |
| OCR empty / Error | Confirm `resources/tessdata/*.traineddata` shipped. Drop in extra Tesseract packs to OCR more languages. Scanned PDFs need OCR; native-text PDFs should not. |
| Linked folder not updating | `notify` needs the folder to exist. Rebuild by removing and adding the link. Hidden `.*` paths are ignored. |
| Disk full during install | Models are multi-GB. Recommendation already filters by RAM; disk is on you. |
| `cargo test` wants a GPU | Default tests do not start llama. `core_smoke` is `#[ignore]` and needs env vars. |

Logs: Settings → Diagnostics. On macOS also `~/Library/Logs/io.rebost.app/`.
