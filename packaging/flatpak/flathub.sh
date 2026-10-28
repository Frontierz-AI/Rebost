#!/usr/bin/env bash
# Write the files of the flathub/ai.rebost.Rebost repository for a release:
# the manifest building the release tag instead of this checkout, and the
# dependency sources generated from that tag's lockfiles.
#
#   git checkout v0.9.9
#   packaging/flatpak/flathub.sh            # writes packaging/flatpak/flathub/
#   packaging/flatpak/flathub.sh --out DIR
#
# Run it on the tagged commit. Then copy the folder's files into a branch of
# the Flathub repository and open a pull request there (see README.md).

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MANIFEST="${ROOT}/packaging/flatpak/ai.rebost.Rebost.yml"
OUT="${ROOT}/packaging/flatpak/flathub"
REPO_GIT="https://github.com/Frontierz-AI/Rebost.git"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --out) OUT="${2:?--out needs a folder}"; shift ;;
    -h | --help)
      sed -n '2,10p' "$0" | sed 's/^# \?//'
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
  shift
done

VERSION="$(sed -n 's/^  "version": "\(.*\)",$/\1/p' "${ROOT}/package.json" | head -n 1)"
TAG="v${VERSION}"
COMMIT="$(git -C "$ROOT" rev-parse HEAD)"
TAG_COMMIT="$(git -C "$ROOT" rev-parse -q --verify "refs/tags/${TAG}^{commit}" || true)"
if [[ "${ALLOW_UNTAGGED:-0}" != 1 && "$TAG_COMMIT" != "$COMMIT" ]]; then
  echo "HEAD is not ${TAG}. Check out the release tag (or set ALLOW_UNTAGGED=1 to try a branch)." >&2
  exit 1
fi

newest="$(sed -n 's/.*<release version="\([^"]*\)".*/\1/p' \
  "${ROOT}/packaging/flatpak/ai.rebost.Rebost.metainfo.xml" | head -n 1)"
if [[ "$newest" != "$VERSION" ]]; then
  echo "The metainfo's newest <release> is ${newest}, not ${VERSION}." >&2
  exit 1
fi

mkdir -p "$OUT"
awk -v url="$REPO_GIT" -v tag="$TAG" -v commit="$COMMIT" '
  /# rebost-source-begin/ {
    indent = $0; sub(/#.*/, "", indent)
    print indent "- type: git"
    print indent "  url: " url
    print indent "  tag: " tag
    print indent "  commit: " commit
    skipping = 1
    next
  }
  /# rebost-source-end/ { skipping = 0; next }
  !skipping { print }
' "$MANIFEST" >"${OUT}/ai.rebost.Rebost.yml"
grep -q "commit: ${COMMIT}" "${OUT}/ai.rebost.Rebost.yml" || {
  echo "the manifest has no rebost-source markers" >&2
  exit 1
}

"${ROOT}/packaging/flatpak/generate-sources.sh" --out "$OUT"
echo "Flathub files for ${TAG} (${COMMIT}) in ${OUT}"
