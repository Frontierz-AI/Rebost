# Rebost

Private AI that works with your files. What happens on your computer stays on your computer.

<p align="center">
  <img src="docs/assets/R.webp" alt="Rebost" width="160" />
</p>

The current release is **0.8.6**. Rebost runs on Mac (Apple chip and Intel) and Windows 10/11. The app is free and MIT licensed. There is no account to create.

[![CI](https://github.com/Frontierz-AI/Rebost/actions/workflows/ci.yml/badge.svg)](https://github.com/Frontierz-AI/Rebost/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![GitHub release](https://img.shields.io/github/v/release/Frontierz-AI/Rebost?include_prereleases)](https://github.com/Frontierz-AI/Rebost/releases)

## For users

Point it at the folder the work already lives in. Install Rebost, take the suggested AI, and ask in Chat. Make a Shelf from a folder you already have when the answer should come from those files.

There is nothing to sign up for. No seats, no usage meter. Each person runs Rebost on their own computer. The app is MIT. The source is public.

### Chat, Shelves, Recipes, and House rules

Chat is the home screen. Ask there. The AI answers on this computer. Attach a file, or drop files onto Chat; they stay on that conversation. Choose a Shelf when the answer should come from a folder. Citations open the source. A new conversation keeps the Shelf you last chose.

A file you attach is read first. If that first look isn't enough, Chat can search again, open a named file, or read the next part of a long one. Open Thinking to see what it looked through. With no Shelf, Rebost uses what the AI already knows. What you already said in the chat still counts. Earlier conversations Chat looks up stay on the same Shelf.

A Shelf is a folder you already keep work in. Create one and attach the folder, or drop files in. You can rename a Shelf; the folder on disk stays put. A new Shelf is created inside Rebost, so macOS does not ask for Documents access. A Shelf stops at 1,000 files. How Chat looks through a Shelf is Off, Light, or Deep. Light and Deep take longer.

A Recipe is a question you save and reuse on any Shelf. You can edit it after you save it. Placeholders use `«…»`. When a placeholder is a document name, Chat can fill it from files on that Shelf.

House rules, in Settings, set tone and language for every chat. Online lets Chat look things up on the web. It stays off until you turn it on. Your files are not sent online.

Settings opens from the menu (⌘, on a Mac, Ctrl+, on Windows). Explore other AIs is there if you want something other than the suggestion. Paste a catalog page or an owner/name to go straight to that AI.

After idle, the first message may say "Warming up…" while the AI gets ready. Stop works then too.

When an answer contains personal information, you can copy it with those identifiers removed. Opening a file shows the text Rebost reads, plus counts of emails, IBANs, tax ids, Social Security numbers, and labeled names. Those counts are not a legal opinion.

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

1. Install Rebost on each computer.
2. On first run, install the suggested AI. You can skip and do it later in Settings. Explore other AIs is there too. Install skips one this computer can't run.
3. Make a Shelf. Link a folder, or drop files in. Rebost reads them on this computer.
4. In Chat, choose that Shelf when the answer should come from those files. Citations open the source.
5. Save a Recipe for anything you will ask again. Set House rules once if you want a shared tone.
6. Leave Online off unless the chat may look things up on the web.

The first chat may take a moment while Rebost gets ready.

### For a small team

Each person installs Rebost. Everyone points a Shelf at the folder the team already shares. There is no account and nothing to administer. The full setup is in [docs/team.md](docs/team.md).

### Frequently asked questions

#### Is it really free?

Yes. The app is MIT licensed and free. There is no paid tier, and nobody assigns a seat or watches a meter.

#### Do I need an account?

No. There is nothing to sign up for and nobody to sign in as. Install it and ask.

#### Do my files leave this computer?

Files you put on a Shelf stay on this computer. Rebost uses the network to search for or install an AI and to check for a newer version, and the web only if you turn Online on.

#### Which AI does it use?

One you install on first run. Rebost suggests one that fits this computer and skips any it can't run. Each AI has its own license, shown before you install.

#### What computer do I need?

A Mac, Apple chip or Intel, or a Windows 10/11 PC. The installer takes care of the rest.

#### Phones? Linux?

Rebost is a desktop app for Mac and Windows. It runs on one computer you can point to.

#### Can a whole team use it?

Yes. Each person installs Rebost, and everyone points a Shelf at the folder the team already shares. [How a small team uses Rebost](docs/team.md).

### Privacy

Files you put on a Shelf stay on this computer. Rebost uses the network when you search for or install an AI from Hugging Face or Ollama, and it may check for a newer version on startup. If that check fails, nothing is shown. On some Windows PCs, the first chat may also download a faster way to run the AI. Online, in Settings, lets Chat look things up on the web; it stays off until you turn it on. Details: [docs/privacy.md](docs/privacy.md).

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

- [docs/faq.md](docs/faq.md): frequently asked questions
- [docs/team.md](docs/team.md): how a small team uses Rebost
- [docs/accessibility.md](docs/accessibility.md): keyboard and VoiceOver notes
- [docs/ui.md](docs/ui.md): colors, buttons, and other UI tokens
- [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md): bundled software notices

## License

[MIT](LICENSE). Trademark: [docs/branding.md](docs/branding.md).
