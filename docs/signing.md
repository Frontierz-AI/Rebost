# Signed installers

GitHub Releases ship signed Mac DMGs and a Windows installer. The Windows file is signed when Azure Artifact Signing is configured; otherwise it is unsigned. Contributors do not need signing credentials.

```bash
pnpm tauri build
```

That writes an unsigned DMG or NSIS for this machine. Gatekeeper will warn on macOS.

Signed builds use a gitignored `.env.signing` file (variable names are in `.env.example`). Those credentials are not in this repository.
