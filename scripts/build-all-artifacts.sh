#!/usr/bin/env bash
# Windows support adapted from barramee27/crossusage (MIT): https://github.com/barramee27/crossusage
# Build Linux (.deb, .rpm, .AppImage) + Windows (.exe + NSIS setup) from a Linux host.
# Requires: bun, Rust, NSIS (makensis) for the Windows installer bundle.
set -euo pipefail
cd "$(dirname "$0")/.."
unset CI
# shellcheck disable=SC1091
source "$(dirname "$0")/load-tauri-signing.sh"
# Some runners set CI=1; `bun run tauri build` can mis-parse that. Prefer: bunx tauri build ...

echo "==> Linux bundles (deb, rpm, appimage)"
NO_STRIP=true bun run tauri build --bundles deb,rpm,appimage

echo "==> Windows (GNU cross-target: openusage.exe + NSIS setup)"
bun run tauri build --target x86_64-pc-windows-gnu

echo ""
echo "Outputs (workspace target/ at repo root; productName OpenUsage + version):"
echo "  deb:       target/release/bundle/deb/OpenUsage_*_amd64.deb (includes openusage + openusage-cli)"
echo "  rpm:       target/release/bundle/rpm/OpenUsage-*.rpm"
echo "  appimage:  target/release/bundle/appimage/OpenUsage_*_amd64.AppImage"
echo "  win exe:   target/x86_64-pc-windows-gnu/release/openusage.exe"
echo "  win dll:   target/x86_64-pc-windows-gnu/release/WebView2Loader.dll  (keep next to openusage.exe for portable GNU builds)"
echo "  win setup: target/x86_64-pc-windows-gnu/release/bundle/nsis/OpenUsage_*_x64-setup.exe"
echo ""
echo "Portable archives (optional, after this script or a normal tauri build):"
echo "  Linux GUI .tar.gz:     ./scripts/build-gui-portable-linux-tarball.sh"
echo "  Windows GUI .zip:      .\\scripts\\build-gui-portable-windows.ps1  (on Windows)"
echo "  Windows GUI .zip:      ./scripts/build-gui-portable-zip-windows-gnu.sh  (on Linux, after step above for win target)"
echo "  Windows CLI-only .zip: ./scripts/build-cli-zip-windows-gnu.sh  (CLI + plugins, no openusage.exe)"
echo "Optional: copy everything into release-artifacts/ with:"
echo "  ./scripts/collect-release-artifacts.sh"
