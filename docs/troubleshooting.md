# Troubleshooting

Chat that will not answer and downloads that will not finish are covered below. Settings → Diagnostics lists the paths Rebost uses on disk. Do not paste document text into a public issue.

| What appears | What to try |
|--------------|-------------|
| Stuck on "Warming up…" | Settings → Diagnostics; the log is on disk. Quit Rebost. Reset only if the AI can be installed again. |
| Chat thinks, then a blank bubble | Try again. On a Snapdragon PC, use the Windows (ARM) download. |
| Download sits at 100% or "Checking the download…" | The check can take a while on a large file. Skip the check and use the file skips it. A mismatch deletes the file; try again. |
| "Couldn't reach the AI catalogs" | Network to huggingface.co / ollama.com. Search is optional; the suggested AI does not need it. |
| Install refused (no checksum) | That listing has no checksum. Pick another AI. |
| File shows Error, or no text | A picture-only PDF needs to be readable as a scan. A normal PDF with selectable text should not. Extra language packs live in `resources/tessdata/` when building from source. |
| Linked folder not updating | The folder must still exist. Remove the link and add it again. Hidden `.*` paths are ignored. |
| Disk full during install | AIs are several GB. A download that doesn't fit stops before it starts. Settings → Where AIs are kept can move them to a folder on another drive. |
| "Rebost can't find the folder where your AIs are kept" | The chosen models folder is on a drive that isn't connected, or was renamed. Connect it and use Try again in Settings, or choose another folder. Nothing installed is forgotten until you choose Rebost's own folder. |
| Tests want a GPU | Default tests do not start the AI. `core_smoke` is ignored and needs env vars. |
| Linux: blank or white window | Usually the graphics driver and WebKitGTK. Rebost already sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` when NVIDIA's driver is loaded. On other hardware, start it from a terminal with `WEBKIT_DISABLE_DMABUF_RENDERER=1 rebost` (or the AppImage path). On NVIDIA, `__NV_DISABLE_EXPLICIT_SYNC=1` can also help; `WEBKIT_DISABLE_COMPOSITING_MODE=1` is the last resort. |
| Linux: the AI is slow | Rebost uses the graphics card through Vulkan. Install the Vulkan loader and drivers (`libvulkan1 mesa-vulkan-drivers` on Debian/Ubuntu, `vulkan-loader mesa-vulkan-drivers` on Fedora, or NVIDIA's driver) and restart Rebost. |
| Linux: linked folder stops updating | A very large folder can exceed the system's file-watch limit. Raise `fs.inotify.max_user_watches` (for example to 524288 with `sysctl`) and restart Rebost. |
| Linux: AppImage does not open | Make it executable (`chmod +x`). Without FUSE, run it with `--appimage-extract-and-run`. |

Logs: Settings → Diagnostics. On macOS also `~/Library/Logs/io.rebost.desktop/`; on Linux `~/.local/share/io.rebost.desktop/logs/`.
