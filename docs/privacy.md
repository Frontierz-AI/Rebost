# Privacy and threat model

## Claims

| Claim | Reality |
|-------|---------|
| Shelf documents are not uploaded | True. Extraction, OCR, indexing, and generation are local. |
| Chat text is not uploaded | True. Completions go to `llama-server` on `127.0.0.1` only. |
| Rebost never uses the network | **False.** Model search/install uses the network. The engine archive is bundled in release builds; GitHub is only hit if that bundle is missing. A startup check may fetch `latest.json` from GitHub Releases; if that fails, the app continues as usual. |
| Privacy Lens is a compliance assessment | **False.** It is a count of detector hits, not a legal opinion. |

## Network egress

| Destination | When | Payload |
|-------------|------|---------|
| `github.com/ggml-org/llama.cpp` | Engine missing from the installer (`tauri dev` without fetch, or a broken bundle) | HTTPS GET of a pinned archive; SHA-256 checked |
| GitHub Releases (`latest.json` on the repo in `Cargo.toml` `package.repository`) | Startup, in the background | HTTPS GET of a small JSON file. Failure is ignored; no UI. |
| `huggingface.co` | Explore / install | Search query, IP, `Rebost/0.2.26` user agent |
| `ollama.com` / `registry.ollama.ai` | Explore / install | Same |
| `127.0.0.1:<port>` | Chat, benchmark | Full prompts including retrieved passages |

No analytics SDK is bundled.

## Data at rest

Unencrypted in the app-data directory (mode `0700` on Unix):

- `extracted/*.md`: full text, including personal information
- Tantivy index
- Conversation JSONL (includes cited passage bodies)
- Cards store **counts**, not the matched strings

Assume anyone with access to your OS user account can read this. Settings → Reset Rebost deletes this app-data directory (and caches). It does not delete Shelf folders you created for your files.

## Trust boundaries

1. **You** (the operator). Linked folders are trusted to the extent you linked them. Symlinks inside a linked tree are **skipped** on scan and cannot be opened through the allowlist.
2. **Documents** are untrusted as *instructions*. Prompts tell the model to treat retrieved text as data. This is not a sandbox.
3. **Model output** is untrusted HTML. The UI sanitizes Markdown (DOMPurify) under a strict CSP. A bug here plus a malicious document could historically reach `invoke()`; treat XSS in the webview as a document-exfil path.
4. **Model weights** are untrusted binaries loaded by llama.cpp. Installs without a SHA-256 are refused. After download, Skip check and proceed uses the file without hashing it.
5. **Compromised renderer.** Tauri commands that open files only accept paths under a known Shelf root.

## House rules

Standing instructions are attacker-controlled if someone else can edit Settings on your Mac. They are injected into the system prompt on every message.
