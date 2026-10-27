#!/usr/bin/env bash
# Write rebost-bin's PKGBUILD and .SRCINFO for a release, and optionally push
# them to the AUR. Run as a normal user on Arch Linux (or in an archlinux
# container): .SRCINFO comes from `makepkg --printsrcinfo`.
#
#   packaging/aur/update.sh 0.9.9                    # writes packaging/aur/rebost-bin/
#   packaging/aur/update.sh 0.9.9 --out /tmp/aur     # writes elsewhere
#   packaging/aur/update.sh 0.9.9 --sums SHA256SUMS  # checksums from a local file
#   packaging/aur/update.sh 0.9.9 --push             # also push to the AUR
#
# Without --sums the checksums come from the release's SHA256SUMS asset. The
# LICENSE checksum is this checkout's LICENSE, so run it from the release tag.
# --pkgrel N rebuilds the same version (packaging-only change); default 1.
#
# --push clones ssh://aur@aur.archlinux.org/rebost-bin.git, commits the two
# files and pushes. The key is AUR_SSH_KEY (private key text, as in CI),
# AUR_SSH_KEY_FILE (a path), or else your own ssh agent and ~/.ssh/config.
# The AUR host key is checked against the fingerprint aur.archlinux.org lists.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TEMPLATE="${ROOT}/packaging/aur/rebost-bin/PKGBUILD"
AUR_REMOTE="ssh://aur@aur.archlinux.org/rebost-bin.git"
AUR_ED25519="SHA256:RFzBCUItH9LZS0cKB5UE6ceAYhBD5C8GeOBip8Z11+4"
REPO_URL="https://github.com/Frontierz-AI/Rebost"

VERSION=""
SUMS=""
OUT="${ROOT}/packaging/aur/rebost-bin"
PKGREL=1
PUSH=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --sums) SUMS="${2:?--sums needs a file}"; shift ;;
    --out) OUT="${2:?--out needs a folder}"; shift ;;
    --pkgrel) PKGREL="${2:?--pkgrel needs a number}"; shift ;;
    --push) PUSH=1 ;;
    -h | --help)
      sed -n '2,19p' "$0" | sed 's/^# \?//'
      exit 0
      ;;
    -*)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
    *) VERSION="$1" ;;
  esac
  shift
done

if [[ -z "$VERSION" ]]; then
  VERSION="$(sed -n 's/^  "version": "\(.*\)",$/\1/p' "${ROOT}/package.json" | head -n 1)"
fi
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]]; then
  echo "not a release version: ${VERSION}" >&2
  exit 2
fi
if [[ ! "$PKGREL" =~ ^[1-9][0-9]*$ ]]; then
  echo "pkgrel must be a positive number: ${PKGREL}" >&2
  exit 2
fi
# pacman versions cannot hold "-"; 0.10.0-beta.1 becomes 0.10.0beta.1,
# which vercmp sorts before 0.10.0.
PKGVER="${VERSION//-/}"

command -v makepkg >/dev/null || {
  echo "makepkg not found: run this on Arch Linux or in an archlinux container." >&2
  exit 1
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

if [[ -z "$SUMS" ]]; then
  SUMS="${work}/SHA256SUMS"
  curl -fsSL --retry 3 -o "$SUMS" "${REPO_URL}/releases/download/v${VERSION}/SHA256SUMS" || {
    echo "the v${VERSION} release has no SHA256SUMS asset" >&2
    exit 1
  }
fi

sum_of() {
  local name="$1" sum
  sum="$(awk -v name="$name" '{ file = $2; sub(/^\*/, "", file) } file == name { print $1 }' "$SUMS")"
  if [[ ! "$sum" =~ ^[0-9a-f]{64}$ ]]; then
    echo "no SHA-256 for ${name} in ${SUMS}" >&2
    exit 1
  fi
  printf '%s' "$sum"
}
SUM_X86_64="$(sum_of "Rebost-${VERSION}-Linux-amd64.deb")"
SUM_AARCH64="$(sum_of "Rebost-${VERSION}-Linux-arm64.deb")"
SUM_LICENSE="$(sha256sum "${ROOT}/LICENSE" | cut -d' ' -f1)"

mkdir -p "$OUT"
sed \
  -e "s/^_pkgver=.*/_pkgver=${VERSION}/" \
  -e "s/^pkgver=.*/pkgver=${PKGVER}/" \
  -e "s/^pkgrel=.*/pkgrel=${PKGREL}/" \
  -e "s/^sha256sums=.*/sha256sums=('${SUM_LICENSE}')/" \
  -e "s/^sha256sums_x86_64=.*/sha256sums_x86_64=('${SUM_X86_64}')/" \
  -e "s/^sha256sums_aarch64=.*/sha256sums_aarch64=('${SUM_AARCH64}')/" \
  -e '/^# The copy in that repository keeps SKIP checksums/d' \
  "$TEMPLATE" >"${work}/PKGBUILD"
for line in "_pkgver=${VERSION}" "pkgver=${PKGVER}" "pkgrel=${PKGREL}" \
  "sha256sums=('${SUM_LICENSE}')" "sha256sums_x86_64=('${SUM_X86_64}')" \
  "sha256sums_aarch64=('${SUM_AARCH64}')"; do
  grep -qxF "$line" "${work}/PKGBUILD" || {
    echo "the PKGBUILD template has no line to set ${line%%=*}" >&2
    exit 1
  }
done
cp "${work}/PKGBUILD" "${OUT}/PKGBUILD"
(cd "$OUT" && makepkg --printsrcinfo >.SRCINFO)
echo "Wrote ${OUT}/PKGBUILD and .SRCINFO for ${VERSION}-${PKGREL}"

[[ "$PUSH" -eq 1 ]] || exit 0

ssh_args=(-o IdentitiesOnly=yes -o "UserKnownHostsFile=${work}/known_hosts" -o StrictHostKeyChecking=yes)
if [[ -n "${AUR_SSH_KEY:-}" ]]; then
  printf '%s\n' "$AUR_SSH_KEY" >"${work}/aur_key"
  chmod 600 "${work}/aur_key"
  ssh_args+=(-i "${work}/aur_key")
elif [[ -n "${AUR_SSH_KEY_FILE:-}" ]]; then
  ssh_args+=(-i "$AUR_SSH_KEY_FILE")
else
  # Agent or ~/.ssh/config decides which key.
  ssh_args=(-o "UserKnownHostsFile=${work}/known_hosts" -o StrictHostKeyChecking=yes)
fi
ssh-keyscan -t ed25519 aur.archlinux.org 2>/dev/null >"${work}/known_hosts"
if ! ssh-keygen -lf "${work}/known_hosts" | grep -qF "$AUR_ED25519"; then
  echo "aur.archlinux.org's host key does not match ${AUR_ED25519}" >&2
  exit 1
fi
export GIT_SSH_COMMAND="ssh ${ssh_args[*]}"

git clone -q "$AUR_REMOTE" "${work}/aur"
cp "${OUT}/PKGBUILD" "${OUT}/.SRCINFO" "${work}/aur/"
cd "${work}/aur"
git add PKGBUILD .SRCINFO
if git diff --cached --quiet; then
  echo "The AUR already has rebost-bin ${PKGVER}-${PKGREL}."
  exit 0
fi
git -c user.name="${AUR_GIT_NAME:-Rebost release}" \
  -c user.email="${AUR_GIT_EMAIL:-info-rebost-app@frontierz.com}" \
  commit -q -m "Update to ${PKGVER}-${PKGREL}"
git push -q origin HEAD:master
echo "Pushed rebost-bin ${PKGVER}-${PKGREL} to the AUR."
