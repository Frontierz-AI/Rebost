#!/bin/zsh
# reset.sh — wipe Rebost app data back to first-run state: settings,
# shelves' derived data, search index, conversations, downloaded AI
# models, engine build, exports, caches, logs, webview data.
#
# Your own files are kept: Shelf folders outside app data (e.g.
# ~/Documents/Rebost) stay on disk. Rebost simply forgets about them.

set -u
APP_ID="io.rebost.app"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "Stopping Rebost…"

pkill -f "Application Support/$APP_ID/engine/" 2>/dev/null
pkill -x rebost 2>/dev/null
pkill -f "$REPO_ROOT/node_modules/.*vite" 2>/dev/null

sleep 1

echo "Removing application state…"
rm -rf ~/Library/Application\ Support/$APP_ID          # shelves data · index · conversations · models · engine · settings
rm -rf ~/Library/Caches/$APP_ID                        # caches
rm -rf ~/Library/WebKit/$APP_ID                        # webview storage
rm -rf ~/Library/Logs/$APP_ID                          # diagnostics logs
rm -f  ~/Library/Preferences/$APP_ID.plist             # window prefs
rm -rf ~/Library/Saved\ Application\ State/$APP_ID.savedState

echo
echo "Done. Rebost is back to first-run state —"
echo "next launch shows onboarding and asks to install the AI model."
echo "Kept on disk: any Shelf folders with your files (e.g. ~/Documents/Rebost)."
