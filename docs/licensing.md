# App vs AI vs reading licenses

The application is MIT. An installed AI carries its own license, shown before the download starts. Extra reading packs, used for picture-only files, are Apache-2.0.

| Layer | License | Where |
|-------|---------|--------|
| Rebost application | MIT | [LICENSE](../LICENSE) |
| llama.cpp binary | MIT | Downloaded, pinned, SHA-256 |
| Tesseract traineddata | Apache-2.0 | `src-tauri/resources/tessdata/` |
| Hugging Face / Ollama weights | **Upstream** (Apache-2.0, MIT, Gemma, and others) | Shown before install |

The MIT grant does **not** cover AI weights. **Gemma 4** is Apache-2.0; **Gemma 3 and earlier** use Gemma Terms. Catalog strings are maintained in `src-tauri/src/engine/catalog.rs`. Keep them accurate.

See [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md).
