# Troubleshooting

If Chat is stuck or a download will not finish, start here. Settings → Diagnostics shows paths on this computer. Do not paste document text into a public issue.

| What you see | What to try |
|--------------|-------------|
| Stuck on "Warming up…" | Settings → Diagnostics; the log is on disk. Quit Rebost. Reset only if you can install the AI again. |
| Download sits at 100% or "Checking the download…" | The check can take a while on a large file. Skip the check and use the file skips it. A mismatch deletes the file; try again. |
| "Couldn't reach the AI catalogs" | Network to huggingface.co / ollama.com. Search is optional; the suggestion on this computer does not need it. |
| Install refused (no checksum) | That listing has no checksum. Pick another AI. |
| File shows Error, or no text | A picture-only PDF needs to be readable as a scan. A normal PDF with selectable text should not. Extra language packs live in `resources/tessdata/` if you build from source. |
| Linked folder not updating | The folder must still exist. Remove the link and add it again. Hidden `.*` paths are ignored. |
| Disk full during install | AIs are several GB. The suggestion already filters by memory; disk space is on you. |
| Tests want a GPU | Default tests do not start the AI. `core_smoke` is ignored and needs env vars. |

Logs: Settings → Diagnostics. On macOS also `~/Library/Logs/io.rebost.desktop/`.
