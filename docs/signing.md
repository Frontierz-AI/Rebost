# Signed installers

GitHub Releases ship signed Mac DMGs and Windows NSIS installers. Contributors do not need signing credentials.

```bash
pnpm tauri build
```

That writes an unsigned DMG or NSIS for this machine. Gatekeeper will warn on macOS.

Signed builds use a gitignored `.env.signing` file (variable names are in `.env.example`). Those credentials are not in this repository.
