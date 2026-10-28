#!/usr/bin/env bash
# Install a Rebost Linux package in a clean distro container (as root) and
# check that it links, opens a window on a virtual display, and that the
# bundled llama.cpp build runs on this glibc. Used by release-linux.yml.
#
#   scripts/smoke-linux.sh deb Rebost-X.Y.Z-Linux-amd64.deb shot.png
#   scripts/smoke-linux.sh rpm Rebost-X.Y.Z-Linux-x86_64.rpm shot.png
#   scripts/smoke-linux.sh appimage Rebost-X.Y.Z-Linux-x86_64.AppImage shot.png
#   scripts/smoke-linux.sh pacman rebost-bin-X.Y.Z-1-x86_64.pkg.tar.zst shot.png
#
# Packages are installed without recommended packages, so the hard
# dependencies alone must be enough to start (no Vulkan loader either).
# The pacman package (the AUR's rebost-bin) must also leave updates to pacman.

set -euo pipefail

KIND="${1:?deb, rpm, appimage or pacman}"
FILE="$(realpath "${2:?package file}")"
SHOT="${3:-}"

# shellcheck disable=SC1091
. /etc/os-release
echo "== ${PRETTY_NAME} $(uname -m), $(ldd --version | head -n 1)"

case "$ID" in
  ubuntu | debian)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq --no-install-recommends \
      ca-certificates dbus desktop-file-utils file imagemagick procps x11-apps x11-utils xvfb >/dev/null
    ;;
  fedora)
    dnf install -y -q --setopt=install_weak_deps=False \
      ImageMagick dbus-daemon dbus-tools desktop-file-utils file findutils procps-ng xorg-x11-server-Xvfb xwd xwininfo
    ;;
  arch)
    # The image is synced first by whoever built the package in it.
    pacman -S --noconfirm --needed --quiet \
      dbus desktop-file-utils file imagemagick procps-ng xorg-server-xvfb xorg-xwd xorg-xwininfo >/dev/null
    ;;
  *)
    echo "No smoke recipe for ${ID}" >&2
    exit 1
    ;;
esac

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

missing_libs() {
  local missing
  missing="$(ldd "$@" 2>&1 | grep 'not found' | sort -u || true)"
  if [[ -n "$missing" ]]; then
    echo "$missing" >&2
    fail "missing shared libraries"
  fi
}

ROOT_DIR=""
case "$KIND" in
  deb)
    apt-get install -y -qq --no-install-recommends "$FILE" >/dev/null
    dpkg -s rebost | sed -n '/^Package/p;/^Version/p;/^Depends/p;/^Recommends/p'
    BIN=/usr/bin/rebost
    ROOT_DIR=/usr/lib
    ;;
  rpm)
    dnf install -y -q --setopt=install_weak_deps=False "$FILE"
    rpm -q --requires rebost | grep -v '^rpmlib' | tr '\n' ' '
    echo
    BIN=/usr/bin/rebost
    ROOT_DIR=/usr/lib
    ;;
  pacman)
    pacman -U --noconfirm "$FILE" >/dev/null
    pacman -Qi rebost-bin | sed -n '/^Name/p;/^Version/p;/^Depends On/p;/^Optional Deps/,/^Required By/p'
    BIN=/usr/bin/rebost
    ROOT_DIR=/usr/lib
    ;;
  appimage)
    # The AppImage carries WebKitGTK but, like any AppImage, expects the
    # desktop's own display stack (Wayland/X11 clients, Mesa EGL/GL/GLES, fonts).
    # GTK 3 and Mesa stand in for a desktop; WebKitGTK itself is not installed.
    case "$ID" in
      ubuntu | debian)
        gtk=libgtk-3-0t64
        apt-cache show "$gtk" >/dev/null 2>&1 || gtk=libgtk-3-0
        apt-get install -y -qq --no-install-recommends "$gtk" libegl1 libegl-mesa0 libgl1 libgles2 libgbm1 >/dev/null
        ;;
      fedora) dnf install -y -q --setopt=install_weak_deps=False gtk3 mesa-libEGL mesa-libGL mesa-libgbm libglvnd-gles ;;
    esac
    chmod +x "$FILE"
    work="$(mktemp -d)"
    (cd "$work" && "$FILE" --appimage-extract >/dev/null)
    ROOT_DIR="$work/squashfs-root"
    echo "-- libraries the AppImage takes from the host"
    LD_LIBRARY_PATH="$ROOT_DIR/usr/lib" ldd "$ROOT_DIR/usr/bin/rebost" |
      awk -v root="$ROOT_DIR" '/=>/ && index($3, root) != 1 {print "  " $1}' | sort
    LD_LIBRARY_PATH="$ROOT_DIR/usr/lib" missing_libs "$ROOT_DIR/usr/bin/rebost"
    echo "-- bundled libvulkan: $(find "$ROOT_DIR" -name 'libvulkan*' | sed "s|$ROOT_DIR||" | tr '\n' ' ')"
    file -L "$FILE" | cut -c1-160
    # No FUSE in a container; the runtime unpacks to /tmp instead.
    export APPIMAGE_EXTRACT_AND_RUN=1
    BIN="$FILE"
    ;;
  *)
    fail "unknown kind ${KIND}"
    ;;
esac

if [[ "$KIND" != appimage ]]; then
  missing_libs "$BIN"
  desktop="$(find /usr/share/applications -name 'Rebost*.desktop' | head -n 1)"
  [[ -n "$desktop" ]] || fail "no .desktop entry"
  cat "$desktop"
  desktop-file-validate "$desktop"
  find /usr/share/icons/hicolor -name 'rebost.png' | sort
fi

echo "== bundled resources"
resources="$(find "$ROOT_DIR" -type d -path '*/Rebost/resources' | head -n 1)"
[[ -n "$resources" ]] || fail "no resources folder under ${ROOT_DIR}"
find "$resources" | sed "s|^${resources}|  resources|" | sort
stray="$(find "$resources" \( -name README.md -o -name .gitkeep \))"
[[ -z "$stray" ]] || fail "repository notes shipped as resources: ${stray}"

echo "== bundled engine"
archive="$(find "$ROOT_DIR" -name 'llama-*-bin-ubuntu-*.tar.gz' | head -n 1)"
[[ -n "$archive" ]] || fail "no llama.cpp archive under ${ROOT_DIR}"
engine="$(mktemp -d)"
echo "$archive"
tar -xzf "$archive" -C "$engine"
server="$(find "$engine" -name llama-server -type f | head -n 1)"
missing_libs "$server"
"$server" --version
"$server" --list-devices

echo "== launch"
export HOME="$(mktemp -d)"
export XDG_RUNTIME_DIR="$(mktemp -d)"
chmod 700 "$XDG_RUNTIME_DIR"
Xvfb :99 -screen 0 1440x900x24 -nolisten tcp >/dev/null 2>&1 &
export DISPLAY=:99
sleep 2
log="$(mktemp)"
dbus-run-session -- "$BIN" >"$log" 2>&1 &
pid=$!
for _ in $(seq 1 30); do
  if xwininfo -root -tree 2>/dev/null | grep -q '"Rebost"'; then
    break
  fi
  kill -0 "$pid" 2>/dev/null || break
  sleep 1
done
# Let the page render before the screenshot.
sleep 8
if ! kill -0 "$pid" 2>/dev/null; then
  cat "$log" >&2
  fail "Rebost exited"
fi
xwininfo -root -tree | grep -i 'rebost' || {
  cat "$log" >&2
  fail "no Rebost window"
}
if [[ -n "$SHOT" ]]; then
  xwd -root -silent | convert xwd:- "png:${SHOT}"
fi
app_log="$HOME/.local/share/io.rebost.desktop/logs/rebost.log"
[[ -f "$app_log" ]] || fail "no app log at ${app_log}"
tail -n 20 "$app_log"
if [[ "$KIND" == pacman ]]; then
  grep -q 'updates come from pacman' "$app_log" || fail "the AUR package still checks for in-app updates"
fi
echo "-- files written under HOME"
find "$HOME" -maxdepth 4 -type d | sed "s|^$HOME|~|" | sort
echo "-- stdout/stderr"
tail -n 40 "$log"
kill "$pid" 2>/dev/null || true
wait "$pid" 2>/dev/null || true

if [[ "$KIND" == rpm || "$KIND" == pacman ]]; then
  echo "== uninstall"
  if [[ "$KIND" == rpm ]]; then
    dnf remove -y -q rebost
  else
    pacman -R --noconfirm rebost-bin >/dev/null
  fi
  [[ ! -e /usr/lib/Rebost ]] || {
    find /usr/lib/Rebost >&2
    fail "uninstall left /usr/lib/Rebost behind"
  }
fi
echo "OK ${KIND} on ${PRETTY_NAME}"
