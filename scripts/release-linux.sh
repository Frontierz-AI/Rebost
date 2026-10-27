#!/usr/bin/env bash
# Linux AppImage, .deb and .rpm for this machine's architecture, with the
# in-app updater signatures. Run on the oldest supported base (Ubuntu 22.04,
# see .github/workflows/release-linux.yml): the binary links that glibc.
#
#   ./scripts/release-linux.sh                # needs TAURI_SIGNING_PRIVATE_KEY
#   ./scripts/release-linux.sh --no-updater   # contributor build, no signatures
#
# Writes public-named copies, their .sig files and SHA256SUMS to
# src-tauri/target/release-linux/, and the updater fragment to
# src-tauri/target/updater/latest-linux-<arch>.json.
# beforeBuildCommand fetches only the llama.cpp pin for this target.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

UPDATER=1
for arg in "$@"; do
  case "$arg" in
    --no-updater) UPDATER=0 ;;
    -h | --help)
      sed -n '2,12p' "$0" | sed 's/^# \?//'
      exit 0
      ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "Linux packages need Linux. From another OS: gh workflow run release-linux.yml" >&2
  exit 1
fi

if [[ -f .env.signing ]]; then
  set -a
  # shellcheck disable=SC1091
  source .env.signing
  set +a
fi

TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
case "$TRIPLE" in
  x86_64-unknown-linux-gnu) ARCH=x86_64 DEB_ARCH=amd64 ;;
  aarch64-unknown-linux-gnu) ARCH=aarch64 DEB_ARCH=arm64 ;;
  *)
    echo "No Linux package for ${TRIPLE}" >&2
    exit 1
    ;;
esac
VERSION="$(node -p "require('./package.json').version")"

CONFIG="$(mktemp)"
cleanup() { rm -f "$CONFIG"; }
trap cleanup EXIT
if [[ "$UPDATER" -eq 1 ]]; then
  if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" && -z "${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ]]; then
    echo "Set TAURI_SIGNING_PRIVATE_KEY (in-app updater), or pass --no-updater." >&2
    exit 1
  fi
  printf '%s\n' '{"bundle":{"createUpdaterArtifacts":true}}' >"$CONFIG"
else
  printf '%s\n' '{}' >"$CONFIG"
fi

# linuxdeploy is itself an AppImage. Containers and CI runners have no FUSE.
export APPIMAGE_EXTRACT_AND_RUN=1

echo "Building Rebost ${VERSION} for ${TRIPLE}…"
pnpm tauri build --bundles appimage,deb,rpm --config "$CONFIG"

BUNDLE="src-tauri/target/release/bundle"
OUT="src-tauri/target/release-linux"
rm -rf "$OUT"
mkdir -p "$OUT"

stage() {
  local pattern="$1" public="$2" found=()
  shopt -s nullglob
  found=($pattern)
  shopt -u nullglob
  if [[ ${#found[@]} -ne 1 ]]; then
    echo "Expected one ${pattern}, found ${#found[@]}" >&2
    exit 1
  fi
  cp -p "${found[0]}" "${OUT}/${public}"
  if [[ "$UPDATER" -eq 1 ]]; then
    cp -p "${found[0]}.sig" "${OUT}/${public}.sig"
  fi
  echo "  ${public}  ($(du -h "${found[0]}" | cut -f1))"
}

echo
echo "Packages:"
stage "${BUNDLE}/appimage/Rebost_${VERSION}_*.AppImage" "Rebost-${VERSION}-Linux-${ARCH}.AppImage"
stage "${BUNDLE}/deb/Rebost_${VERSION}_${DEB_ARCH}.deb" "Rebost-${VERSION}-Linux-${DEB_ARCH}.deb"
stage "${BUNDLE}/rpm/Rebost-${VERSION}-*.${ARCH}.rpm" "Rebost-${VERSION}-Linux-${ARCH}.rpm"
(cd "$OUT" && sha256sum Rebost-*.AppImage Rebost-*.deb Rebost-*.rpm >SHA256SUMS && cat SHA256SUMS)

if [[ "$UPDATER" -eq 1 ]]; then
  echo
  node scripts/latest-json.mjs --bundle-dir "$BUNDLE" --triple "$TRIPLE"
fi
