# Security policy

## Supported versions

The current release is **0.8.8**.

## What Rebost does with data

Rebost is MIT licensed and published by Frontierz, the copyright holder named in [LICENSE](LICENSE). There is no account to create.

Chat, reading Shelf documents, search, and answers all run on the machine where Rebost is installed. Those documents and the installed AI stay on that machine.

Rebost uses the network to **search for or install an AI** (Hugging Face, Ollama). Release builds include what runs the AI. GitHub is contacted if that piece is missing (typically `pnpm tauri dev` without `pnpm fetch-engine`), and on some Windows machines a faster copy may be downloaded the first time Chat runs.

Those requests send a query string, IP address, and the user agent `Rebost/0.8.8 (local-first open-source desktop AI; https://github.com/Frontierz-AI/Rebost)`. They do not include Shelf documents.

With Online on in Settings, Chat can also look things up on the public web. Those lookups leave the machine directly and do not go through Rebost, and they carry a user agent naming the project and a contact address. Chat is asked not to put private details in them.

A threat model lives in [docs/privacy.md](docs/privacy.md).

## Reporting a vulnerability

**Do not open a public issue** for a vulnerability that could leak local documents or run code in the window.

Email **info-rebost-app@frontierz.com**, or open a [private advisory](https://github.com/Frontierz-AI/Rebost/security/advisories/new) on GitHub. Either way, include:

- A short description
- Affected version / commit
- Steps to reproduce
- Impact (especially anything that reaches extracted text, conversations, or the network)

We will acknowledge within a few days and work on a fix before any public write-up.

## Scope

In scope: the Rebost desktop app, commands that the window can call, AI download and install, Markdown shown in the window, path handling for Shelves and conversations.

Out of scope: third-party AI weights, the upstream runtime, Hugging Face / Ollama availability, physical access to an unlocked machine.
