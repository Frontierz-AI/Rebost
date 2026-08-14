# Security policy

## Supported versions

The current release is **0.2.26**.

## What Rebost does with data

Chat, document extraction, OCR, retrieval, and generation run on the local machine. Your files and conversation text stay on this computer.

Rebost uses the network when you **search for or install an AI model** (Hugging Face, Ollama). Release builds ship the pinned `llama.cpp` engine; GitHub (`ggml-org/llama.cpp`) is contacted only if that bundle is missing, typically `pnpm tauri dev` without `pnpm fetch-engine`.

Those requests send a query string, IP address, and a `Rebost/0.2.26` user agent. They do not include Shelf documents.

A threat model lives in [docs/privacy.md](docs/privacy.md).

## Reporting a vulnerability

**Do not open a public issue** for a vulnerability that could leak local documents or execute code in the webview.

Email **pau@frontierz.com** with:

- A short description
- Affected version / commit
- Steps to reproduce
- Impact (especially anything that reaches extracted text, conversations, or the network)

You can also use [GitHub private vulnerability reporting](https://github.com/Frontierz-AI/Rebost/security/advisories/new).

We will acknowledge within a few days and work on a fix before any public write-up.

## Scope

In scope: the Rebost desktop app, Tauri commands, local engine download/install, Markdown rendering in the webview, path handling for shelves and conversations.

Out of scope: third-party model weights, llama.cpp itself, Hugging Face / Ollama availability, physical access to an unlocked machine.
