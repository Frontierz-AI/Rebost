# Rebost

Private AI that lives with your files and never leaves them.

<p align="center">
  <img src="docs/assets/R.webp" alt="Rebost" width="160" />
</p>

The current release is **0.2.26**. Rebost runs on macOS (Apple Silicon and Intel) and Windows 10/11 (x64 with Vulkan, ARM64). Chat, shelves, and models stay on the machine that runs the app.

[![CI](https://github.com/Frontierz-AI/Rebost/actions/workflows/ci.yml/badge.svg)](https://github.com/Frontierz-AI/Rebost/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![GitHub release](https://img.shields.io/github/v/release/Frontierz-AI/Rebost?include_prereleases)](https://github.com/Frontierz-AI/Rebost/releases)

## For users

### What it is

Rebost is a desktop app for talking to a language model on your own computer. You can chat with the model alone, or add your files to a Shelf and ask questions about them.

- **Chat** is the home screen. Attach a file when you need to. Pick a Shelf to answer from your files; citations open the source. With no Shelf, Rebost uses the model's general knowledge. Conversation memory still applies either way. After idle, the first message may say "Warming up..." while the model loads.
- **Shelves** are named collections of your files. Drag in PDF, Word, or spreadsheets, or link a folder on disk (watched for changes). Opening a file shows extracted text and **counts** of detected personal information (emails, IBANs, Spanish tax ids). Those counts are not a compliance assessment.
- **Recipes** are saved prompts for work you repeat. Placeholders use `«…»`.
- **House rules** are standing tone and language instructions. They stay out of retrieved document excerpts.

When an answer contains personal information, you can copy it with those identifiers removed.

### Install

Download the installer for your machine from [GitHub Releases](https://github.com/Frontierz-AI/Rebost/releases).

| Machine | Download |
|---------|----------|
| Mac with an Apple chip | Mac (Apple chip) |
| Intel Mac | Mac (Intel) |
| Windows 10/11 | Windows |

On a Mac, open the file and drag Rebost into Applications. On Windows, run the installer.

Windows 11 includes the WebView2 Runtime. On Windows 10, the installer downloads the [Evergreen WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) when it is missing.

Windows x64 needs a Vulkan driver from your GPU vendor (NVIDIA, AMD, or Intel). Windows ARM64 is not on the Releases page yet; it can be built from source and runs the model on the CPU.

Each installer includes the llama.cpp engine for that OS and architecture only (~11–33 MB). After a later GitHub Release, Rebost can update itself from the sidebar when it can reach GitHub. If GitHub is unreachable, nothing is shown.

### First run

1. Onboarding: the privacy promise, then install the model recommended for this computer. You can skip and do it later in Settings.
2. The first chat unpacks the llama.cpp engine that shipped in the installer. GitHub downloads and SHA-256 checks apply when that bundle is missing (`tauri dev`, or a broken copy). Signed Mac installers re-sign Mach-O files inside the archive for notarization, so that copy is not compared to the GitHub pin SHA.
3. Optional: create a Shelf, drop files or link a folder. Rebost reads them on this computer.
4. In Chat, pick that Shelf when the answer should come from your files. Citations open the source.

### Privacy

Documents and chat are processed on this computer. Rebost uses the network when you search for or install a model from Hugging Face or Ollama. Release builds ship the pinned llama.cpp engine; GitHub is only contacted if that bundle is missing. On startup it may also fetch `latest.json` from GitHub Releases; if that fails, nothing is shown. Details: [docs/privacy.md](docs/privacy.md).

The app is MIT. **Model weights are licensed separately** and shown before install. [docs/licensing.md](docs/licensing.md).

If something looks like a leak of local files, read [SECURITY.md](SECURITY.md) before filing a public issue. For bugs, use [GitHub issues](https://github.com/Frontierz-AI/Rebost/issues). For a stuck engine or download, see [docs/troubleshooting.md](docs/troubleshooting.md).

## For developers

### Requirements

- macOS (Apple Silicon or Intel), or Windows 10/11 (x64 with Vulkan, or ARM64). Linux is not a supported platform.
- Disk space for a GGUF model (several GB)
- Rust 1.97.1, Node 22+, pnpm 11 (`rust-toolchain.toml` and `.nvmrc`)
- macOS: Xcode Command Line Tools
- Windows: MSVC toolchain. Windows 11 includes WebView2. On Windows 10, install the [Evergreen WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) before `pnpm tauri dev`. The NSIS installer downloads it when it is missing.
- First OCR build: CMake and a C++ compiler (Xberg compiles Tesseract)

### Run locally

```bash
pnpm install
pnpm tauri dev
```

`pnpm tauri dev` without a prior `pnpm fetch-engine` downloads the pinned llama.cpp archive on first chat. Run `pnpm fetch-engine` once if you want that step to stay offline.

### Checks and installers

```bash
pnpm check                         # svelte-check
pnpm format:check
cargo test --manifest-path src-tauri/Cargo.toml
pnpm test                          # Vitest
pnpm tauri build                   # unsigned DMG / NSIS
```

`pnpm tauri build` writes a DMG under `src-tauri/target/release/bundle/dmg/` (Apple Silicon or Intel, matching this machine unless you pass `--target`), or an NSIS installer under `src-tauri/target/release/bundle/nsis/`. Without signing credentials the installer is unsigned, and Gatekeeper will warn on macOS. See [docs/signing.md](docs/signing.md).

`./scripts/reset.sh` (macOS) or `./scripts/reset.ps1` (Windows) returns the app to first-run (Shelf folders on disk are kept). Settings → Reset Rebost does the same from the running app. `cargo run --manifest-path src-tauri/Cargo.toml --example seed` loads demo data.

Contributing, commands, and architecture: [CONTRIBUTING.md](CONTRIBUTING.md), [docs/development.md](docs/development.md), [docs/releasing.md](docs/releasing.md), [docs/architecture.md](docs/architecture.md), [docs/engine.md](docs/engine.md).

## Project docs

- [docs/accessibility.md](docs/accessibility.md) — keyboard and VoiceOver notes
- [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) — bundled OCR data, llama.cpp, crates

## License

[MIT](LICENSE). Copyright © 2026 Frontierz. Main developer: [Pau Garcia-Mila](https://github.com/paugm).

Trademark: [docs/branding.md](docs/branding.md).
