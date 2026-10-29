# Releases

Signed installers are on [GitHub Releases](https://github.com/Frontierz-AI/Rebost/releases). Installed copies check for a newer version on startup and offer an update when one exists. A failed check is ignored.

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

`release-linux.yml` also builds the AUR package `rebost-bin` from its own x86_64 .deb on `archlinux:latest`: `packaging/aur/update.sh` writes the PKGBUILD with both architectures' checksums from the run's `SHA256SUMS`, makepkg builds it, namcap checks it, pacman installs it, and `scripts/smoke-linux.sh pacman` opens it and checks that it leaves updates to pacman. The artifact `rebost-aur-x86_64` holds the package, PKGBUILD and .SRCINFO.

## AUR and Flathub

The publisher uploads a `SHA256SUMS` asset covering every installer. When a release that is not a prerelease is published, `linux-stores.yml` runs on its own (or `gh workflow run linux-stores.yml -f tag=vX.Y.Z`):

- **AUR:** rewrites `rebost-bin` for the tag from that `SHA256SUMS`, builds it from the release's own .deb files, runs namcap, installs it and runs the smoke test, then pushes `PKGBUILD` and `.SRCINFO` to `ssh://aur@aur.archlinux.org/rebost-bin.git` with the `AUR_SSH_KEY` secret. Without the secret it only uploads them as the artifact `aur-vX.Y.Z`. Setup: [packaging/aur/README.md](../packaging/aur/README.md).
- **Flathub:** writes the manifest for the tag's commit and regenerates `cargo-sources.json` and `node-sources.json` from the tag's lockfiles (`packaging/flatpak/flathub.sh`), uploaded as the artifact `flathub-vX.Y.Z`. A maintainer opens the pull request with those files on `flathub/ai.rebost.Rebost`; Flathub's rules do not allow an AI tool to open it. See [packaging/flatpak/README.md](../packaging/flatpak/README.md), including Flathub's policy on AI-written manifests.

Each version bump also adds a `<release>` to `packaging/flatpak/ai.rebost.Rebost.metainfo.xml` (newest first); `pnpm test` fails when its newest release is not the version in `package.json`. `flatpak.yml` builds the Flatpak from the checkout on pull requests that change it or its lockfiles, lints it with Flathub's linter, opens it on Xvfb, and uploads `rebost-flatpak-x86_64` and `rebost-flatpak-aarch64`.

Installs from the AUR or Flathub never run the in-app updater (`src-tauri/src/distribution.rs`): a Flatpak is recognised by `/.flatpak-info`, and the AUR package installs `/usr/lib/Rebost/package-manager`. Neither asks GitHub for `latest.json`, so its `linux-*` entries only serve the GitHub packages.

The in-app updater endpoint is `{Cargo.toml package.repository}/releases/latest/download/latest.json`. Change the repository URL in `src-tauri/Cargo.toml` if the GitHub repo moves; do not hardcode it elsewhere. Installed copies fetch that file without credentials, so the GitHub repository must be public or the check is ignored. `pnpm tauri dev` does not check.
