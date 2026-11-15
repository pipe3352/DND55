#!/usr/bin/env bash
# Copies the user-facing documents into a release folder (Windows line endings for .txt).
set -euo pipefail
DEST="$1"
cd "$(dirname "$0")/.."
cp WINDOWS_INSTALLATION_GUIDE.md "$DEST/"
sed 's/$/\r/' packaging/windows/README.txt > "$DEST/README.txt"
{ cat LICENSE-MIT; echo; echo "----"; echo; cat packaging/windows/THIRD_PARTY.txt; } | sed 's/$/\r/' > "$DEST/LICENSE.txt"
