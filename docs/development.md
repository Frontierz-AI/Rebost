# Development

See [CONTRIBUTING.md](../CONTRIBUTING.md) for prerequisites, first 30 minutes, and PR expectations. Installer cuts: [releasing.md](releasing.md).

## Commands

| Command | What |
|---------|------|
| `pnpm install` | Frontend deps |
| `pnpm tauri dev` | Dev app + Vite |
| `pnpm tauri build` | Fetches the host engine archive, then a DMG (macOS) or NSIS (Windows). Unsigned unless `.env.signing` is loaded. |
| `pnpm fetch-engine` | Download/stage the pinned llama.cpp archive (`--all`, `--triple=…`) |
| `pnpm check` | `svelte-check` |
| `pnpm format:check` | Prettier |
| `pnpm test` | Vitest |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Rust tests |
| `just check` / `just test` / `just reset` / `just seed` | Wrappers |

## Environment

Copy `.env.example`. Vite variables:

- `VITE_START_VIEW`: `chat` \| `shelves` \| `recipes` \| `settings`
- `VITE_START_SHELF=first`: open the first shelf in Shelves view

## Reset and seed

`./scripts/reset.sh` (macOS) or `./scripts/reset.ps1` (Windows) deletes the Rebost app-data directory and related caches. Settings → Reset Rebost does the same from inside the running app (type `DELETE` to confirm). Your `~/Documents/Rebost` (or equivalent) folders stay.

`cargo run --manifest-path src-tauri/Cargo.toml --example seed -- [--fresh] [--model GGUF]` fills a demo library. Quit Rebost first (Tantivy writer is exclusive).

## Logging

- App: stdout + OS log dir (`~/Library/Logs/io.rebost.app/` on macOS; `%LOCALAPPDATA%\io.rebost.app\logs` on Windows)
- Engine: `logs/engine.log` under app data
- Default level: Info (`tauri-plugin-log`)

## Toolchain

Pinned: Rust 1.97.1, pnpm 11.21.0, Node 22 (`.nvmrc`).
