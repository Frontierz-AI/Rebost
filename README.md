# Rebost

Private AI that works with your files. What happens in your computer stays in your computer.

<p align="center">
  <img src="docs/assets/R.webp" alt="Rebost" width="160" />
</p>

The current release is **0.8.4**. Rebost runs on macOS (Apple Silicon and Intel) and Windows 10/11. Chat, Shelves, and the AI stay on the machine that runs the app.

[![CI](https://github.com/Frontierz-AI/Rebost/actions/workflows/ci.yml/badge.svg)](https://github.com/Frontierz-AI/Rebost/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![GitHub release](https://img.shields.io/github/v/release/Frontierz-AI/Rebost?include_prereleases)](https://github.com/Frontierz-AI/Rebost/releases)

## For users

### What it is

Rebost is a desktop app. You talk to an AI on your own computer. You can chat alone, or add files to a Shelf and ask about them.

Chat is the home screen. Attach a file when you need to, or drop files onto Chat; they stay on that conversation. Choose a Shelf when the answer should come from your files; citations open the source. A new conversation keeps the Shelf you last chose. A file you attach is read first. If that first look isn't enough, Chat can search again, open a named file, or read the next part of a long one. Open Thinking to see what it looked through. With no Shelf, Rebost uses what the AI already knows. A setting lets Chat look things up on the web; it stays off until you turn it on. Your files are not sent online. What you already said in the chat still counts, with or without a Shelf. Earlier conversations Chat looks up stay on the same Shelf. After idle, the first message may say "Warming up…" while the AI gets ready. Stop works then too.

A new chat can start from a Recipe. You can edit a Recipe after you save it.

Add PDFs, Word files, or spreadsheets to a Shelf, or link a folder so new files show up here. You can rename a Shelf; the folder on disk stays put. A new Shelf is created inside Rebost, so macOS does not ask for Documents access. A Shelf stops at 1,000 files. How Chat looks through a Shelf is Off, Light, or Deep. Light and Deep take longer. Opening a file shows the text Rebost reads, plus counts of emails, IBANs, tax ids, Social Security numbers, and labeled names. Those counts are not a legal opinion.

Recipes save prompts you reuse. Placeholders use `«…»`. When a placeholder is a document name, Chat can fill it from files on that Shelf.

House rules set tone and language for every chat. Settings opens from the menu (⌘, on a Mac, Ctrl+, on Windows). Explore other AIs is there too, if you want something other than the suggestion. Paste a catalog page or an owner/name to go straight to that AI.

When an answer contains personal information, you can copy it with those identifiers removed.

Each conversation has a face. You can rename it. Download is at the top of the thread.

### Install

Download the installer for your machine from [GitHub Releases](https://github.com/Frontierz-AI/Rebost/releases).

| Machine | Download |
|---------|----------|
| Mac with an Apple chip | Mac (Apple chip) |
| Intel Mac | Mac (Intel) |
| Windows 10/11 | Windows |
| Windows on ARM | Windows (ARM) |

On a Mac, open the file and drag Rebost into Applications. On Windows, run the installer.

Windows 11 already has what Rebost needs to draw its windows. On Windows 10, the installer may add a small Microsoft component ([WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)) if it is missing.

On a regular Windows PC (not ARM), you need a graphics driver that supports Vulkan, from NVIDIA, AMD, or Intel.

If you already have Rebost, a later release can show up in the sidebar when GitHub is reachable. If not, nothing is shown.

### First run

1. Onboarding: the privacy promise, then install the AI recommended for this computer. You can skip and do it later in Settings. Explore other AIs is there too. Install skips one this computer can't run.
2. The first chat may take a moment while Rebost gets ready.
3. Optional: create a Shelf, drop files or link a folder. Rebost reads them on this computer.
4. In Chat, choose that Shelf when the answer should come from your files. Citations open the source.

### Privacy

Your files and the AI stay on this computer. Rebost uses the network when you search for or install an AI from Hugging Face or Ollama, and it may check for a newer version on startup. If that check fails, nothing is shown. On some Windows PCs, the first chat may also download a faster way to run the AI. A setting lets Chat look things up on the web; it stays off until you turn it on. Details: [docs/privacy.md](docs/privacy.md).

The app is MIT. The AI you install has its own license, shown before you install. [docs/licensing.md](docs/licensing.md).

If something looks like a leak of local files, read [SECURITY.md](SECURITY.md) before filing a public issue. For bugs, use [GitHub issues](https://github.com/Frontierz-AI/Rebost/issues). If Chat is stuck getting ready, see [docs/troubleshooting.md](docs/troubleshooting.md).

## For developers

### Requirements

- macOS (Apple Silicon or Intel), or Windows 10/11 (x64 with Vulkan, or ARM64). Linux is not a supported platform.
- Disk space for a GGUF model (several GB)
- Rust 1.97.1, Node 22+, pnpm 11 (`rust-toolchain.toml` and `.nvmrc`)
- macOS: Xcode Command Line Tools
- Windows: MSVC toolchain. Windows 11 includes WebView2. On Windows 10, install the [Evergreen WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) before `pnpm tauri dev`. The NSIS installer downloads it when it is missing.
- First OCR build: CMake and a C++ compiler (xberg compiles Tesseract)

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

- [docs/accessibility.md](docs/accessibility.md): keyboard and VoiceOver notes
- [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md): bundled software notices

## License

[MIT](LICENSE). Trademark: [docs/branding.md](docs/branding.md).
