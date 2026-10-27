# Releases

Signed installers are on [GitHub Releases](https://github.com/Frontierz-AI/Rebost/releases). Installed copies check for a newer version on startup and offer an update when one exists. A failed check is ignored. The app-data layout may change without a migration.

## Version

The same version must appear in:

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml` (package `version` only)
- `src-tauri/Cargo.lock` (`name = "rebost"`)

Current-version references in README, SECURITY, CHANGELOG, the bug template, and `docs/privacy.md` should match. Accessibility's "verified against" version changes only after that verification is performed. The HTTP user agent reads its version from `CARGO_PKG_VERSION`, so it follows the bump without an edit. Historical CHANGELOG sections stay as they were.

Finish source changes, synchronization, and tracked release documentation before building. Run the full verification gate on the final source. Every installer in a release must correspond to the published commit; rebuild after source or build-input changes. Record manual and hardware checks from `experience-quality.md` separately from automated checks.

## What a contributor can build

```bash
pnpm tauri build
```

That writes an unsigned DMG, NSIS, or AppImage/.deb/.rpm for this machine. Gatekeeper will warn on macOS. Windows ARM64 NSIS is `pnpm tauri build --target aarch64-pc-windows-msvc` on ARM Windows, or the `windows-11-arm` job in `release-windows.yml`.

Signed installers need credentials that are not in this repository. See [signing.md](signing.md).

## Linux packages

`release-linux.yml` builds the AppImage, .deb, and .rpm for x86_64 (`ubuntu-24.04`) and aarch64 (`ubuntu-24.04-arm`) inside an `ubuntu:22.04` container, so the binary links glibc 2.35 and runs on Ubuntu 22.04+, Debian 12+, Fedora 43+, and other distributions of that age. It does not run on push. Dispatch it on the release commit:

```bash
gh workflow run release-linux.yml
```

It also runs on pull requests that change Linux packaging (the workflow, `scripts/release-linux.sh`, `scripts/smoke-linux.sh`, `src-tauri/tauri.linux.conf.json`, `src-tauri/linux/`), without publishing anything.

Each build job runs `scripts/release-linux.sh`, which builds with `createUpdaterArtifacts` and uploads the artifact `rebost-linux-<arch>`:

- `release-linux/Rebost-X.Y.Z-Linux-<x86_64|aarch64>.AppImage`, `Rebost-X.Y.Z-Linux-<amd64|arm64>.deb`, `Rebost-X.Y.Z-Linux-<x86_64|aarch64>.rpm`, their `.sig` files, and `SHA256SUMS`
- `updater/latest-linux-<arch>.json`, the updater fragment with `linux-<arch>-appimage`, `-deb`, and `-rpm` entries pointing at those public names

The smoke jobs install each package without recommended packages in clean `ubuntu:22.04`, `ubuntu:24.04`, `ubuntu:26.04`, `debian:12`, `debian:13`, `fedora:43`, and `fedora:44` containers (and the AppImage on Ubuntu 24.04 and Fedora 44, with only GTK 3 and Mesa installed to stand in for a desktop), check that nothing is missing in `ldd` for the app and the bundled `llama-server`, open the app on Xvfb, and upload a screenshot. A release attaches the six packages under their public names and merges both fragments into `latest.json` with `node scripts/latest-json.mjs --combine` (or the maintainer's publish helper). The in-app updater picks the entry for the format it was installed from; deb and rpm updates run `dpkg -i` or `rpm -U` through `pkexec`.

On a Linux machine, `./scripts/release-linux.sh --no-updater` builds the same packages without the updater key.

The in-app updater endpoint is `{Cargo.toml package.repository}/releases/latest/download/latest.json`. Change the repository URL in `src-tauri/Cargo.toml` if the GitHub repo moves; do not hardcode it elsewhere. Installed copies fetch that file without credentials, so the GitHub repository must be public or the check is ignored. `pnpm tauri dev` does not check.
