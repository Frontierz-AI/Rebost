# AUR: rebost-bin

`rebost-bin/PKGBUILD` repackages the release's `.deb` (x86_64 and aarch64) for Arch Linux and the distributions built on it (EndeavourOS, CachyOS, Manjaro, Omarchy):

```bash
yay -S rebost-bin
```

The .deb is the binary the Debian and Ubuntu packages install, linked against the system's WebKitGTK 4.1, which Arch ships as `webkit2gtk-4.1`. The AppImage was not used: it carries its own, older WebKitGTK and GTK and needs FUSE. The package adds `/usr/lib/Rebost/package-manager`, which makes the app leave updates to pacman (About says so) instead of running its own updater.

The copy in git is the template: `update.sh` sets `pkgver`, `pkgrel` and the checksums from a release's `SHA256SUMS` and writes `.SRCINFO` with `makepkg --printsrcinfo`. Between releases the checksums are `SKIP`.

```bash
packaging/aur/update.sh 0.9.9                    # PKGBUILD and .SRCINFO in rebost-bin/
packaging/aur/update.sh 0.9.9 --out /tmp/aur     # elsewhere
packaging/aur/update.sh 0.9.9 --push             # and push to the AUR
```

It needs `makepkg`, so it runs on Arch Linux or in an `archlinux` container, as a normal user.

## CI

- `release-linux.yml` (pull requests that change Linux packaging, and every dispatch) builds `rebost-bin` from that run's own `.deb` in `archlinux:latest`, checks that the committed `.SRCINFO` matches the template, runs namcap on the PKGBUILD and the package, installs it with pacman, and runs `scripts/smoke-linux.sh pacman` (opens the app on Xvfb, checks it leaves updates to pacman, uninstalls). The package and its PKGBUILD are the artifact `rebost-aur-x86_64`.
- `linux-stores.yml` runs when a release is published: it builds the package from the release's own files the same way and pushes `PKGBUILD` and `.SRCINFO` to `ssh://aur@aur.archlinux.org/rebost-bin.git` when the `AUR_SSH_KEY` secret is set.

aarch64 is declared and its checksum filled in, but CI only builds x86_64: there is no official Arch Linux image for ARM runners.

## Setup (once)

1. Create an account at <https://aur.archlinux.org/register> (Arch's own account system).
2. Make a key for it, with no passphrase so CI can use it: `ssh-keygen -t ed25519 -C "rebost AUR" -f ~/.ssh/aur_rebost -N ''`.
3. In the AUR account, My Account → SSH Public Key: paste `~/.ssh/aur_rebost.pub`.
4. Store the private key as the repository secret `AUR_SSH_KEY` (Settings → Secrets and variables → Actions):
   `gh secret set AUR_SSH_KEY --repo Frontierz-AI/Rebost < ~/.ssh/aur_rebost`
5. The first push creates the package. Pushing by hand from an Arch machine also works: `AUR_SSH_KEY_FILE=~/.ssh/aur_rebost packaging/aur/update.sh X.Y.Z --push`.
