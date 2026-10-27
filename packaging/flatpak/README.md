# Flatpak (Flathub)

`ai.rebost.Rebost.yml` builds Rebost from source with no network access, the way Flathub builds apps. CI builds it from this checkout (`.github/workflows/flatpak.yml`), lints it with Flathub's linter, opens it on Xvfb, for x86_64 and aarch64, and uploads the bundles as the artifacts `rebost-flatpak-x86_64` and `rebost-flatpak-aarch64`.

```bash
flatpak install --user --bundle rebost.flatpak
flatpak run ai.rebost.Rebost
```

## App ID

`ai.rebost.Rebost`, the reverse of the project site `rebost.ai`. Flathub requires a domain the submitter controls, verifies it with a file on that domain, and rejects IDs that end in `.desktop`, so `io.rebost.desktop` cannot be used (and `rebost.io` is not the project's domain).

Only the Flatpak uses the new ID: the desktop entry, icon and metainfo are named after it. Inside the app the Tauri identifier stays `io.rebost.desktop`, so nothing changes for the Mac, Windows and Linux packages or their users' data. A Flatpak keeps its data in `~/.var/app/ai.rebost.Rebost/data/io.rebost.desktop`, separate from a .deb, .rpm or AppImage install in `~/.local/share/io.rebost.desktop`; Shelves and AIs do not carry over between the two.

## What the manifest does

- **Runtime:** GNOME 51. WebKitGTK 4.1, which Tauri 2 uses, is in the GNOME runtime and not in the Freedesktop one; Flathub asks for the latest runtime at submission. Its Freedesktop SDK 26.08 has `glslc` for llama.cpp's Vulkan shaders.
- **Rust:** `org.freedesktop.Sdk.Extension.rust-stable`, with `--ignore-rust-version` while the extension is on Rust 1.98 (the project pins 1.99.1; this release builds with 1.98.1).
- **Frontend:** `org.freedesktop.Sdk.Extension.node24`, pnpm's own Linux binary (`@pnpm/exe.linux-*`), and `node-sources.json`, which fills an offline pnpm store from `pnpm-lock.yaml`.
- **Crates:** `cargo-sources.json` vendors every crate in `src-tauri/Cargo.lock`.
- **Tesseract and Leptonica:** xberg-tesseract's build script downloads them; the manifest declares the same archives with their checksums and seeds its cache (`TESSERACT_RS_CACHE_DIR`), so the build stays offline.
- **llama.cpp:** built at the tag pinned in `src-tauri/src/engine/pin.rs` (`ENGINE_BUILD`), with the Vulkan and CPU backends as loadable modules and CPU variants per processor generation. It is packed under the upstream Linux archive names, so the app unpacks it like the bundled engine of the other packages. The same build without the Vulkan module is the CPU fallback, so the Flatpak never downloads an engine.
- **Updates:** inside a Flatpak the app finds `/.flatpak-info`, never checks GitHub, and About says updates come from the software centre.

`src/lib/packaging.test.ts` fails when the metainfo's newest release is not the version in `package.json`, or the manifest's llama.cpp tag or archive names drift from `pin.rs`.

## Permissions

| Permission                                                 | Why                                                                                                                                                                                                                                                                                                                                              |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `--socket=wayland`, `--socket=fallback-x11`, `--share=ipc` | The window.                                                                                                                                                                                                                                                                                                                                      |
| `--device=dri`                                             | Vulkan for the AI, and WebKitGTK's GPU rendering.                                                                                                                                                                                                                                                                                                |
| `--share=network`                                          | Downloading an AI from Hugging Face, and web lookups when they are turned on. Documents are not uploaded.                                                                                                                                                                                                                                        |
| `--filesystem=home`                                        | A Shelf is a folder Rebost reads and keeps watching. A folder chosen through the document portal arrives as a FUSE path under `/run/user/…/doc` that delivers no inotify events, so new and changed files would never reach the Shelf. The folder chosen for AIs is written to. Read-only access would break that and the move of installed AIs. |
| `--filesystem=/media`, `--filesystem=/run/media`           | The same for external drives, where people keep documents and large AI files (Debian and Ubuntu mount them under `/media`, Fedora and Arch under `/run/media`).                                                                                                                                                                                  |

## Dependency sources

```bash
packaging/flatpak/generate-sources.sh
```

writes `cargo-sources.json` and `node-sources.json` next to the manifest (gitignored) with flatpak-builder-tools at a pinned commit. Rerun it after a change to `Cargo.lock` or `pnpm-lock.yaml`.

## Before submitting: Flathub's generative AI policy

Flathub's [requirements](https://docs.flathub.org/docs/for-app-authors/requirements) say that Flathub manifests must not contain AI-generated or AI-assisted content, that other AI-generated material in a submission must be disclosed with the affected parts and their approximate extent, and that AI tools must not open or automate submission pull requests or write their descriptions.

The manifest in this folder was written with an AI assistant. It is tested in CI and documents what the build needs, but it cannot be submitted as it is: the manifest sent to Flathub has to be written by a person. The metainfo, desktop entry and the app's own code are covered by the disclosure rule.

## Submitting

1. Write the Flathub manifest yourself. `flathub.sh` turns a manifest in this folder into the release-tag version: it replaces the `type: dir` source between the `rebost-source-begin`/`-end` markers with `type: git` at the tag and commit, and regenerates the dependency sources from that tag.

   ```bash
   git checkout vX.Y.Z
   packaging/flatpak/flathub.sh    # writes packaging/flatpak/flathub/
   ```

   The `Linux stores` workflow writes the same files as the artifact `flathub-vX.Y.Z` for each published release.

2. Build and lint it locally:

   ```bash
   flatpak install -y flathub org.flatpak.Builder
   flatpak run --command=flathub-build org.flatpak.Builder --install packaging/flatpak/flathub/ai.rebost.Rebost.yml
   flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest packaging/flatpak/flathub/ai.rebost.Rebost.yml
   flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo repo
   ```

3. Fork [flathub/flathub](https://github.com/flathub/flathub/fork) with "Copy the master branch only" unchecked, then:

   ```bash
   git clone --branch=new-pr git@github.com:YOUR_USER/flathub.git && cd flathub
   git checkout -b ai.rebost.Rebost new-pr
   cp ../Rebost/packaging/flatpak/flathub/* .
   git add . && git commit -m "Add ai.rebost.Rebost" && git push -u origin ai.rebost.Rebost
   ```

   Open the pull request against the **`new-pr`** branch (not `master`), titled `Add ai.rebost.Rebost`. Explain the permissions (the table above) and disclose the AI-assisted parts. Comment `bot, build` for a test build once review comments are answered.

4. After the merge Flathub creates `flathub/ai.rebost.Rebost` and invites the submitter: turn on GitHub two-factor authentication and accept within a week.
5. Verify the domain: log in at flathub.org, open the Developer Portal, select the app, open Verification, and publish the token it shows at `https://rebost.ai/.well-known/org.flathub.VerifiedApps.txt` (one token per line, HTTPS).

## Each release

After the release is published, the `Linux stores` workflow uploads `flathub-vX.Y.Z` (the manifest on the tag's commit and the regenerated sources). Open a pull request with those files on `flathub/ai.rebost.Rebost`; Flathub builds it and publishes on merge. Add the release to `<releases>` in the metainfo as part of the version bump.
