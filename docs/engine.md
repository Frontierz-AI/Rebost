# Local engine

The language model is llama.cpp `llama-server`, managed as a child process. It is not a Tauri `externalBin` sidecar: official builds ship dylibs (macOS) and DLLs (Windows), so Rebost bundles the **pinned archive** as a resource and unpacks it into app data.

Each installer contains **one** archive for that OS/arch. There is no universal Mac or Windows binary. llama.cpp does not ship fat libraries, and a universal `.app` would carry two engines.

## Pin

`src-tauri/src/engine/pin.rs`:

- `ENGINE_BUILD` (e.g. `b10418`)
- `ENGINE_PINS`: one GitHub archive URL + SHA-256 per OS/arch

| Host | Archive | Accelerator | Typical archive size |
|------|---------|-------------|----------------------|
| macOS arm64 | `macos-arm64.tar.gz` | Metal | ~11 MB |
| macOS x64 | `macos-x64.tar.gz` | Metal | ~11 MB |
| Windows x64 | `win-vulkan-x64.zip` | Vulkan | ~33 MB |
| Windows arm64 | `win-cpu-arm64.zip` | CPU | ~12 MB |
| Linux x64 / arm64 | `ubuntu-vulkan-*.tar.gz` | Vulkan (not a supported platform; pin exists so the crate compiles on Ubuntu CI). There is no Linux installer overlay. | ~26–32 MB |

CI HEAD-checks every URL. `node scripts/fetch-engine.mjs` downloads the pin for this build target into `src-tauri/resources/engine/` (gitignored). `pnpm tauri build` runs that fetch first.

Cross-compile:

```bash
node scripts/fetch-engine.mjs --triple=x86_64-apple-darwin
pnpm tauri build --target x86_64-apple-darwin
```

Signed Mac installers **codesign every Mach-O inside the archive** (notary unpacks `.tar.gz`), then notarize. llama.cpp still unpacks on first chat; it is not a Tauri sidecar.

`--all` fills the cache with macOS + Windows pins; only the current host file is staged for bundling.

Windows x64 needs a working Vulkan driver. Windows arm64 is CPU-only in this pin.

## First run

1. If `engine/<build>/llama-server` (`.exe` on Windows) exists, use it.
2. Else if `REBOST_ENGINE_ARCHIVE` is set, copy that archive and SHA-256-verify it (tests / air-gapped).
3. Else if the installer bundled the pin, copy that archive. Signed Mac builds re-sign Mach-O inside it for notarization, so that copy is **not** checked against the GitHub pin SHA.
4. Else download the host pin and verify SHA-256 (`tauri dev` without a prior fetch).
5. Unpack tar.gz or zip (path-safe), chmod `0755` on Unix, spawn on `127.0.0.1`. Working directory is the engine folder so Windows DLLs load.

Logs live under the OS app-data directory (`logs/engine.log`). Settings → Diagnostics shows the path, not the log body.

## Offline

A release build already contains the archive. For tests, point `REBOST_ENGINE_ARCHIVE` at a copy of the **host** pin (same SHA). Models still have to be present under `models/`.

## Warmup

`warm_engine` and Chat both call `ensure_ready`. The UI shows "Warming up..." while weights load. Closing Rebost kills the process (and leftover PIDs from crashed dev builds).
