# Architecture

Rebost is a desktop app: a Svelte 5 UI in a Tauri 2 webview, and a Rust core that owns documents, search, and the language-model process.

```
Chat / Shelves / Recipes / Settings
            │  invoke + events (rebost://…)
            ▼
     Tauri commands (thin adapters)
            │
    ┌───────┼────────┬──────────┬─────────┐
    ▼       ▼        ▼          ▼         ▼
  shelf   ingest   search     chat     engine
  watcher extract  tantivy   prompts   llama-server
          PII      gate                download
```

## Product pieces

**Chat.** Retrieve (optional Shelf + conversation memory), gate, local `llama-server`, streamed answer with `[S1]` citations.

**Shelves.** Named libraries. Each has a managed folder for imports plus optional linked folders that `notify` watches.

**Ingest.** Xberg extraction (PDF, Office, email, HTML, and the rest), optional OCR from whatever `*.traineddata` packs ship in `resources/tessdata`, Card YAML, passages, Tantivy.

**Privacy Lens.** Counts of emails, phones, IBANs, Spanish tax ids, and similar. Counts only on the Card. Full text still lives in `extracted/*.md`, the index, and conversation JSONL. Redact-on-copy is a display action, not encryption at rest.

**Recipes.** Saved prompts. House rules are standing instructions, never mixed into retrieved excerpts.

**Engine.** A pinned llama.cpp `llama-server` archive is bundled in the installer, unpacked into app data, and spawned on `127.0.0.1`. Not a Tauri `externalBin` sidecar. GitHub downloads are SHA-256 verified. Signed Mac copies are re-signed for notarization, so they skip the GitHub pin SHA; see [engine.md](engine.md).

## Data on disk

The OS app-data directory (see `src-tauri/src/paths.rs`): macOS `~/Library/Application Support/io.rebost.app/`, Windows `%APPDATA%\io.rebost.app\`.

```
shelves/<id>/{cards,extracted,documents.json}
search/tantivy/
conversations/          threads.json + one JSONL per thread
models/                 GGUF files (one active model)
engine/<build>/         llama-server
recipes.json
settings.json
logs/engine.log
```

The layout can change without a migration. Settings → Reset Rebost (or `scripts/reset.sh`) returns to first-run without deleting Shelf folders on disk.

## Module map (`src-tauri/src/`)

| Module | Job |
|--------|-----|
| `commands` | IPC only |
| `shelf` | library + linked folders + watcher |
| `ingest` | queue, extract, card, passages |
| `search` | Tantivy + multilingual stem + relevance gate |
| `chat` | orchestration + prompts + conversations |
| `engine` | process, download, catalog, stream split |
| `pii` | Privacy Lens |
| `updater` | Silent GitHub `latest.json` check; in-app install |
| `recipes` / `settings` / `reset` / `paths` / `ids` | support |

The frontend contract is `src/lib/api.ts`. Event names are `rebost://engine|download|ingest|shelf-stats|shelves|chat|update|update-progress`.
