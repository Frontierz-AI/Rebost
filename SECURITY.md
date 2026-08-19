# Security policy

## Supported versions

The current release is **0.8.2**.

## What Rebost does with data

Chat, reading your files, search, and answers run on this computer. Your files and the AI stay here.

Rebost uses the network when you **search for or install an AI** (Hugging Face, Ollama). Release builds include what runs the AI. GitHub is contacted if that piece is missing (typically `pnpm tauri dev` without `pnpm fetch-engine`), and on some Windows machines a faster copy may be downloaded the first time you chat.

Those requests send a query string, IP address, and a `Rebost/0.8.2` user agent. They do not include Shelf documents.

With Online on in Settings, Chat can also look things up on the public web from this computer. Those lookups do not go through Rebost. Chat is asked not to put private details in them.

A threat model lives in [docs/privacy.md](docs/privacy.md).

## Reporting a vulnerability

**Do not open a public issue** for a vulnerability that could leak local documents or run code in the window.

Email **pau@frontierz.com** with:

- A short description
- Affected version / commit
- Steps to reproduce
- Impact (especially anything that reaches extracted text, conversations, or the network)

We will acknowledge within a few days and work on a fix before any public write-up.

## Scope

In scope: the Rebost desktop app, commands that the window can call, AI download and install, Markdown shown in the window, path handling for Shelves and conversations.

Out of scope: third-party AI weights, the upstream runtime, Hugging Face / Ollama availability, physical access to an unlocked machine.
