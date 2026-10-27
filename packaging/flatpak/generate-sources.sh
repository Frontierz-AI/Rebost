#!/usr/bin/env bash
# Write the offline dependency sources the Flatpak manifest lists:
#   cargo-sources.json  every crate in src-tauri/Cargo.lock
#   node-sources.json   every package in pnpm-lock.yaml, and the pnpm store layout
#
#   packaging/flatpak/generate-sources.sh             # next to the manifest
#   packaging/flatpak/generate-sources.sh --out DIR   # elsewhere (flathub.sh)
#
# Uses flatpak-builder-tools at a pinned commit, in a Python virtualenv under
# the user cache. Needs python3 (with venv) and git. Rerun after any change to
# Cargo.lock or pnpm-lock.yaml.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="${ROOT}/packaging/flatpak"
# flatpak-builder-tools master on 2026-10-25: pnpm lockfile v9 with pnpm 12 stores.
TOOLS_COMMIT=9d86d336a39b9af38b86f114b6ce5820fe260f91
# pnpm 12 keeps the v11 store layout.
PNPM_STORE=v11

while [[ $# -gt 0 ]]; do
  case "$1" in
    --out) OUT="${2:?--out needs a folder}"; shift ;;
    -h | --help)
      sed -n '2,11p' "$0" | sed 's/^# \?//'
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
  shift
done
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"

CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/rebost-flatpak/${TOOLS_COMMIT}"
TOOLS="${CACHE}/flatpak-builder-tools"
VENV="${CACHE}/venv"
if [[ ! -d "${TOOLS}/.git" ]]; then
  rm -rf "$TOOLS"
  git init -q "$TOOLS"
  git -C "$TOOLS" fetch -q --depth 1 https://github.com/flatpak/flatpak-builder-tools.git "$TOOLS_COMMIT"
  git -C "$TOOLS" checkout -q FETCH_HEAD
fi
if [[ ! -x "${VENV}/bin/flatpak-node-generator" ]]; then
  python3 -m venv "$VENV"
  "${VENV}/bin/pip" install -q aiohttp tomlkit "${TOOLS}/node"
fi

echo "Cargo sources…"
"${VENV}/bin/python" "${TOOLS}/cargo/flatpak-cargo-generator.py" \
  "${ROOT}/src-tauri/Cargo.lock" -o "${OUT}/cargo-sources.json"

echo "pnpm sources…"
"${VENV}/bin/flatpak-node-generator" pnpm "${ROOT}/pnpm-lock.yaml" \
  --pnpm-store-version "$PNPM_STORE" -o "${OUT}/node-sources.json"

ls -l "${OUT}/cargo-sources.json" "${OUT}/node-sources.json"
