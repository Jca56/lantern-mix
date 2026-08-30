#!/usr/bin/env bash
# Link the desktop entry and icon into the user's XDG dirs so the app shows up
# in launchers and docks. Re-run after moving the repo. Nothing is copied: the
# repo and ~/Pictures/Icons stay the source of truth.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
apps="$HOME/.local/share/applications"
icons="$HOME/.local/share/icons/hicolor/scalable/apps"
icon_src="$HOME/Pictures/Icons/Apps/lantern-mix.svg"
mkdir -p "$apps" "$icons"
ln -sfn "$here/assets/lantern-mix.desktop" "$apps/lantern-mix.desktop"
ln -sfn "$icon_src" "$icons/lantern-mix.svg"
echo "linked $apps/lantern-mix.desktop"
echo "linked $icons/lantern-mix.svg -> $icon_src"
command -v update-desktop-database >/dev/null && update-desktop-database "$apps" 2>/dev/null || true
