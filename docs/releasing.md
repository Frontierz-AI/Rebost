# Releases

Signed installers are published on [GitHub Releases](https://github.com/Frontierz-AI/Rebost/releases). Installed copies check that URL for `latest.json` on startup and offer an in-app update when a newer version is listed. A failed check (offline, 404, GitHub down) is ignored. The app-data layout may change without a migration.

## Version

The same version must appear in:

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml` (package `version` only)
- `src-tauri/Cargo.lock` (`name = "rebost"`)

Docs that quote it (README, SECURITY, `docs/accessibility.md`, CHANGELOG, the bug template, the HTTP user agent in `engine/process.rs`, `docs/privacy.md`) should match. Historical CHANGELOG sections stay as they were.

## What a contributor can build

```bash
pnpm tauri build
```

That writes an unsigned DMG or NSIS for this machine. Gatekeeper will warn on macOS. Linux is not a supported platform. llama.cpp pins for Linux exist only so the crate compiles on Ubuntu CI; there is no Linux installer.

Signed installers need credentials that are not in this repository. See [signing.md](signing.md).

The in-app updater endpoint is `{Cargo.toml package.repository}/releases/latest/download/latest.json`. Change the repository URL in `src-tauri/Cargo.toml` if the GitHub repo moves; do not hardcode it elsewhere.
