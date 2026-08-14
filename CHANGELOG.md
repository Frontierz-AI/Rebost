# Changelog

All notable changes to this project are documented in this file.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
The app-data layout may change without a migration.

## [0.2.26] - 2026-08-14

First public release. Rebost is a desktop app: you talk to a language model on this computer, alone or against files you put on a Shelf.

### Added

- Chat via a bundled llama.cpp engine. Attach a file when you need to, or pick a Shelf so answers come from your files with citations. Conversation memory applies either way. After idle, the first message may say "Warming up..." while the model loads.
- Shelves: named collections. Drag in PDF, Word, or spreadsheets, or link a folder (watched for changes). Opening a file shows extracted text and counts of detected personal information (emails, IBANs, Spanish tax ids). Those counts are not a compliance assessment.
- Recipes: saved prompts with `«…»` placeholders. House rules are standing tone and language instructions and stay out of retrieved excerpts.
- Copy an answer with personal identifiers removed when the reply contains them.
- First-run onboarding: the privacy promise, then a catalog model that fits this computer. Settings later shows two more suggestions that are not already installed.
- Model search on Hugging Face (download counts, original publishers first, newest first). More info on a hit opens a card with publisher, downloads, file, and license. After a download you can skip the hash check and use the file as-is.
- In-app updater: a silent GitHub `latest.json` check on startup, a sidebar cue when a newer version exists, and an Update window that downloads and installs it.
- Settings → Reset Rebost wipes app data, models, and caches after you type DELETE. Shelf folders on disk are kept.
- Show in folder on the document drawer and citation panel.
- Installers for Mac (Apple Silicon and Intel) and Windows x64. Each ships the llama.cpp pin for that OS and architecture. Windows ARM64 can be built from source and runs the model on the CPU. Linux is not a supported platform.
- English UI. Confirm before deleting conversations, Recipes, and Shelves.

### Security

- Model installs without a SHA-256 are refused. `source` / `reference` and GGUF names are validated. A local `REBOST_ENGINE_ARCHIVE` is SHA-256-checked like a network download. Signed Mac engine copies skip the GitHub pin SHA after Mach-O re-sign (same as `docs/engine.md`).
- HTTPS-only client for Hugging Face, Ollama, and GitHub engine downloads. Local llama-server stays on loopback HTTP.
- Markdown sanitization and a strict Content-Security-Policy.
- Path-id validation and filesystem allowlists for Open / Reveal. Linked-folder scans skip symlinks.
- App-data directories are created mode `0700` on Unix.
- Diagnostics do not send the engine log body to the webview.

[0.2.26]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.2.26
