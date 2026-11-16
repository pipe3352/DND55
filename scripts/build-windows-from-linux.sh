#!/usr/bin/env bash
# Cross-compiles DND55 for 64-bit Windows from Linux and packages:
#   dist/DND55/DND55.exe, dist/DND55-Windows-Portable.zip, dist/DND55-Setup.exe,
#   dist/DND55-Relay-Windows.zip (optional relay server) and dist/DND55-Source.zip (for Method B)
# Requirements (Debian/Ubuntu): rustup, `rustup target add x86_64-pc-windows-gnu`,
#   apt install gcc-mingw-w64-x86-64 nsis zip
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
TARGET=x86_64-pc-windows-gnu
rustup target add "$TARGET" >/dev/null
cargo build --release -p dnd_desktop -p dnd_relay --target "$TARGET" --target-dir target/windows
EXE=target/windows/$TARGET/release/DND55.exe

rm -rf dist && mkdir -p dist/DND55
cp "$EXE" dist/DND55/DND55.exe
x86_64-w64-mingw32-strip dist/DND55/DND55.exe || true
scripts/stage-docs.sh dist/DND55

# Sanity check: the exe must only import DLLs that ship with Windows.
x86_64-w64-mingw32-objdump -p dist/DND55/DND55.exe | grep "DLL Name" | sort -u > dist/dll-imports.txt
if grep -viE "kernel32|user32|gdi32|advapi32|shell32|ole32|oleaut32|ws2_32|bcrypt|ntdll|imm32|winmm|dwmapi|uxtheme|comctl32|opengl32|setupapi|crypt32|secur32|shlwapi|propsys|version|dbghelp|hid|userenv|api-ms-win|d3d12|dxgi|d3dcompiler|comdlg32|winspool|msvcrt|cfgmgr32|powrprof|iphlpapi|netapi32|mswsock|dbgcore|xinput|combase|rpcrt4|synchronization|pdh|psapi|uiautomationcore|bcryptprimitives|ksuser|avrt|mmdevapi" dist/dll-imports.txt; then
  echo "WARNING: unexpected DLL dependency above (would need to be shipped)" >&2
fi

# Portable zip: portable.txt makes DND55 store saves next to the exe in data\.
mkdir -p dist/portable/DND55
cp dist/DND55/* dist/portable/DND55/
printf 'This file makes DND55 portable: saves go to the data folder next to DND55.exe.\r\nDelete it to use %%APPDATA%%\\DND55 instead.\r\n' > dist/portable/DND55/portable.txt
(cd dist/portable && zip -qr ../DND55-Windows-Portable.zip DND55)
rm -rf dist/portable

makensis -V2 -DVERSION="$VERSION" -DSTAGE="$PWD/dist/DND55" -DOUTFILE="$PWD/dist/DND55-Setup.exe" packaging/windows/installer.nsi
# Relay server for Windows (optional; most people run it on a Linux VPS, see docs/RELAY.md).
mkdir -p dist/relay/DND55-Relay
cp target/windows/$TARGET/release/dnd55-relay.exe dist/relay/DND55-Relay/
x86_64-w64-mingw32-strip dist/relay/DND55-Relay/dnd55-relay.exe || true
sed 's/$/\r/' docs/RELAY.md > dist/relay/DND55-Relay/RELAY.md
(cd dist/relay && zip -qr ../DND55-Relay-Windows.zip DND55-Relay)
rm -rf dist/relay

# Source snapshot for building from source (Method B of the guide).
git archive --format=zip --prefix=dnd55/ -o dist/DND55-Source.zip HEAD

(cd dist && sha256sum DND55/DND55.exe DND55-Windows-Portable.zip DND55-Setup.exe DND55-Relay-Windows.zip DND55-Source.zip > SHA256SUMS.txt)
ls -la dist
