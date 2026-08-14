# Contributing to Rebost

Rebost is a Tauri 2 + Svelte 5 + Rust desktop app. Supported platforms: macOS (Apple Silicon and Intel) and Windows 10/11 (x64 with Vulkan, ARM64). Linux is not a supported platform.

## First 30 minutes

1. Install [Rust](https://rustup.rs/) 1.97.1 (`rust-toolchain.toml` pins this), Node 22+ (`.nvmrc`), [pnpm](https://pnpm.io) 11, and [just](https://github.com/casey/just).
2. macOS: Xcode Command Line Tools. Windows: MSVC toolchain. First OCR build needs **CMake** and a C++ compiler (Xberg compiles Tesseract). Windows x64 also needs a Vulkan driver from the GPU vendor. Windows 11 includes WebView2; on Windows 10 install the [Evergreen WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) before `pnpm tauri dev`.
3. From the repo root:

```bash
pnpm install
just check          # svelte-check, Prettier, cargo fmt, clippy
just test           # cargo test + Vitest
pnpm tauri dev
```

`pnpm tauri build` fetches the pinned llama.cpp archive for that target (~11–33 MB) and ships it in the bundle. Without signing credentials the DMG / NSIS is unsigned. See [docs/releasing.md](docs/releasing.md).

For `pnpm tauri dev`, run `pnpm fetch-engine` once if you want first chat to stay offline. Otherwise the running app downloads the pin.

## One test

```bash
cargo test --manifest-path src-tauri/Cargo.toml recipes::tests::old_contract_key_terms_id_is_renamed
cargo test --manifest-path src-tauri/Cargo.toml --test pipeline_test markdown_contract
pnpm exec vitest run src/lib/focus-trap.test.ts
```

Optional full-loop smoke (downloads nothing if you pass local files):

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test core_smoke -- --ignored --nocapture
```

Needs `REBOST_ENGINE_ARCHIVE` (the **host** archive from [docs/engine.md](docs/engine.md)) and `REBOST_TEST_MODEL` (see `.env.example`). Network catalog tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test model_catalog -- --ignored
```

CI runs frontend checks and `cargo fmt` / `clippy` / `test` / `deny` on Ubuntu. Linux engine pins exist so the crate compiles there; there is no Linux installer. Windows is covered by the manual release workflow, not every push.

## TypeScript versions

`pnpm check` (`svelte-check`) uses **TypeScript 6** (`typescript` in `package.json`). `@typescript/native` is a TypeScript 7 preview for editors. Version-mismatch warnings between those two are expected.

## Dev utilities

- `./scripts/reset.sh` (macOS) or `./scripts/reset.ps1` (Windows): wipe Rebost app data (Shelf folders on disk are kept). Settings → Reset Rebost does the same from the running app.
- `cargo run --manifest-path src-tauri/Cargo.toml --example seed -- [--model PATH] [--fresh]`: demo library (quit the app first)
- `VITE_START_VIEW=shelves VITE_START_SHELF=first pnpm tauri dev`: land on a specific screen

Logs: Settings → Diagnostics (paths only; the engine log body stays on disk). On macOS also `~/Library/Logs/io.rebost.app/`. Engine stdout is `logs/engine.log` under app data.

## Where to change things

| Task | Start here |
|------|------------|
| Tauri commands | `src-tauri/src/commands/` and `src/lib/api.ts` |
| Model catalog / recommend | `src-tauri/src/engine/catalog.rs` |
| Engine URL / SHA matrix | `src-tauri/src/engine/pin.rs` |
| Add a file format | `docs/ingest-formats.md` |
| Chat prompts | `src-tauri/src/chat/prompts.rs` |
| UI copy | the view (English) |
| In-app updates | `src-tauri/src/updater.rs`, `src/lib/views/UpdateWindow.svelte` |

## Pull requests

- Open a PR against `main`. Direct pushes to `main` are for maintainers.
- Issues tagged `good first issue` or `help wanted` are a reasonable place to start.
- Commit messages are **prose that explains why**, not Conventional Commit prefixes. Match the existing history.
- Include a test when you change ingest, retrieval, PII, downloads, or chat orchestration.
- Do not add a CLA. A `Signed-off-by` line (DCO) is welcome and optional.

## Platform policy

Supported platforms: macOS and Windows 10/11. Release installers bundle one llama.cpp archive per OS/arch (`engine/pin.rs`). Linux URLs are in that matrix so the crate compiles on Linux CI. There is no Linux installer. See [docs/engine.md](docs/engine.md).

## License

By contributing you agree that your work is licensed under the MIT License in `LICENSE`.
