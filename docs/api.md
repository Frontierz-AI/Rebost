# HTTP / event contract

Canonical types live in Rust. The TypeScript mirror is `src/lib/api.ts`. Prefer `camelCase` on the wire. On-disk JSON that used `snake_case` is still accepted via serde aliases.

Handlers live under `src-tauri/src/commands/`, plus `about.rs` and `updater.rs`. New commands need a handler, a registration in `lib.rs` `generate_handler!`, and an `api.ts` wrapper in the same PR.

## Events

| Name | Payload |
|------|---------|
| `rebost://engine` | `EngineStatus` |
| `rebost://download` | `DownloadEvent` |
| `rebost://ingest` | per-document status |
| `rebost://shelf-stats` | `{ shelfId, stats }` |
| `rebost://shelves` | empty object (reload list) |
| `rebost://chat` | stream machine (`queued` … `done` / `error`) |
| `rebost://update` | `{ version, currentVersion, notes? }` when a newer release exists |
| `rebost://update-progress` | `{ event: started \| progress \| finished }` while installing |

## Commands

| Command | Args | Returns |
|---------|------|---------|
| `shelves_list` | — | `ShelfView[]` |
| `shelf_create` | `name` | `ShelfView` |
| `shelf_remove` | `shelfId` | `()` |
| `shelf_add_linked` | `shelfId` | `ShelfView \| null` (null if the folder picker was cancelled) |
| `shelf_remove_source` | `shelfId`, `sourceId` | `()` |
| `shelf_import_paths` | `shelfId`, `paths` | queued file count |
| `shelf_import_dialog` | `shelfId` | queued file count |
| `shelf_documents` | `shelfId` | `DocumentMeta[]` |
| `document_card` | `shelfId`, `docId` | `Card` |
| `document_text` | `shelfId`, `docId` | extracted text (capped) |
| `document_reprocess` | `shelfId`, `docId` | `()` |
| `open_original` | `path` | `()` (allowlisted Shelf paths only) |
| `reveal_item` | `path` | `()` (allowlisted Shelf paths only) |
| `threads_list` | — | `ThreadMeta[]` |
| `thread_create` | `shelfId?` | `ThreadMeta` |
| `thread_messages` | `threadId` | `StoredMessage[]` |
| `thread_set_shelf` | `threadId`, `shelfId?` | `()` |
| `thread_delete` | `threadId` | `()` |
| `chat_send` | `threadId`, `text`, `shelfId?` | `()` (answer arrives on `rebost://chat`) |
| `chat_cancel` | `messageId` | `()` |
| `warm_engine` | — | `()` |
| `engine_status` | — | `EngineStatus` |
| `machine_profile` | — | `MachineView` |
| `active_model` | — | `ActiveModel \| null` (Rust command; the UI reads the model from `settings_get`) |
| `models_search` | `query` | `ModelSearchResult[]` |
| `model_install` | `source`, `reference`, `name`, `license?` | `()` (progress on `rebost://download`) |
| `download_cancel` | `id` | `()` |
| `download_skip_verify` | `id` | `()` (model downloads only; finishes install without hashing) |
| `settings_get` | — | `SettingsView` |
| `settings_set_house_rules` | `text` | `()` |
| `settings_finish_onboarding` | — | `()` |
| `settings_reset_workspace` | `confirmation` (`DELETE`) | `()` (writes a marker, stops the engine, restarts; wipe runs on next launch) |
| `redact_text` | `text` | redacted string |
| `text_has_pii` | `text` | `bool` |
| `diagnostics` | — | `Diagnostics` (no engine log body) |
| `recipes_list` | — | `Recipe[]` |
| `recipe_create` | `name`, `prompt` | `Recipe` |
| `recipe_delete` | `id` | `()` |
| `recipes_restore_defaults` | — | `Recipe[]` |
| `about_info` | — | `AboutInfo` |
| `show_about_window` | — | `()` |
| `open_external` | `link` | `()` |
| `update_info` | — | `AppUpdate \| null` (null unless a newer GitHub release was confirmed) |
| `show_update_window` | — | `()` |
| `install_update` | — | `()` (progress on `rebost://update-progress`; app restarts) |
