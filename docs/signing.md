# Signed installers

GitHub Releases ship signed Mac DMGs and Windows installers (regular PC and ARM). The Windows files are signed when Azure Artifact Signing is configured; otherwise they are unsigned. Contributors do not need signing credentials.

Upstream llama.cpp binaries are unsigned, and antivirus heuristics flag an unsigned `llama-server.exe` (Avast reported IDP.Generic on 0.9.0). A signed Windows release therefore also signs `llama-server.exe` and every DLL in the bundled engine zip: `beforeBuildCommand` runs `scripts/sign-engine-windows.mjs`, which repacks the zip through `scripts/sign-engine-windows.ps1` when `release-windows.ps1` sets `REBOST_SIGN_ENGINE=1`. The ARM runner cannot sign, so the `engine-arm64` job in `release-windows.yml` signs the ARM64 zip on x64 and the ARM build picks it up through `REBOST_SIGNED_ENGINE_DIR`. If engine signing fails, the build warns and bundles the upstream zip; the installer is still signed. Engines downloaded at warmup (CUDA, Adreno OpenCL, the x64 CPU fallback, and the ARM64 CPU build for an x64 copy on ARM) come straight from llama.cpp and stay unsigned.

```bash
pnpm tauri build
```

That writes an unsigned DMG or NSIS for this machine. Gatekeeper will warn on macOS.

Signed builds use a gitignored `.env.signing` file (variable names are in `.env.example`). Those credentials are not in this repository.
