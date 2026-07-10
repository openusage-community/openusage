#!/usr/bin/env bash
# Windows support adapted from barramee27/crossusage (MIT): https://github.com/barramee27/crossusage
# Single-file portable Windows GUI from Linux.
# Builds one self-extracting openusage.exe that contains:
#   openusage_gui.exe + openusage-cli.exe + WebView2Loader.dll + resources/ + icons/
#
# By default this script runs `tauri build` first so it never packages a stale dev-style binary.
# Set OPENUSAGE_SKIP_TAURI_BUILD=1 only when you intentionally want to reuse target/ output.
#
# Output filename (repo root, copied to releases/ if present):
#   openusage_<version>_windows_amd64_onefile.exe
#
# Avoid clobbering uploads / same-name conflicts:
#   OPENUSAGE_ONEFILE_TAG=mybuild
#     -> openusage_<version>_windows_amd64_onefile_mybuild.exe
#   OPENUSAGE_UNIQUE_ONEFILE=1
#     -> openusage_<version>_windows_amd64_onefile_<UTCdatetime>_<gitsha>.exe
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

TARGET="x86_64-pc-windows-gnu"
VERSION="$(node -p "require('./package.json').version")"

SUFFIX=""
if [[ -n "${OPENUSAGE_ONEFILE_TAG:-}" ]]; then
  # Sanitize for a filename (alphanumeric, dot, dash, underscore).
  SAFE_TAG="$(printf '%s' "${OPENUSAGE_ONEFILE_TAG}" | tr -c 'A-Za-z0-9._-' '_')"
  SUFFIX="_${SAFE_TAG}"
elif [[ "${OPENUSAGE_UNIQUE_ONEFILE:-0}" == "1" ]]; then
  TS="$(date -u +%Y%m%dT%H%M%SZ)"
  GIT="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo nogit)"
  SUFFIX="_${TS}_${GIT}"
fi

OUT="${ROOT}/openusage_${VERSION}_windows_amd64_onefile${SUFFIX}.exe"

need_file() {
  if [[ ! -f "$1" ]]; then
    echo "Missing $1" >&2
    echo "Try again without OPENUSAGE_SKIP_TAURI_BUILD=1 so this script can run tauri build first." >&2
    exit 1
  fi
}

if [[ "${OPENUSAGE_SKIP_TAURI_BUILD:-0}" != "1" ]]; then
  bun run tauri build --target "${TARGET}"
else
  echo "==> OPENUSAGE_SKIP_TAURI_BUILD=1 — rebuilding GUI with embedded frontend (custom-protocol) …"
  bun run bundle:plugins
  bun run build
  cargo build --release -p openusage -p openusage-cli --target "${TARGET}"
fi

need_file "${ROOT}/target/${TARGET}/release/openusage.exe"
need_file "${ROOT}/target/${TARGET}/release/openusage-cli.exe"
need_file "${ROOT}/target/${TARGET}/release/WebView2Loader.dll"

if [[ ! -d "${ROOT}/src-tauri/resources/bundled_plugins" ]]; then
  echo "Missing src-tauri/resources/bundled_plugins — run: bun run bundle:plugins" >&2
  exit 1
fi

OPENUSAGE_ONEFILE=1 cargo build --release -p openusage-win-launcher --target "${TARGET}"
cp -f "${ROOT}/target/${TARGET}/release/openusage-win-launcher.exe" "$OUT"

echo "==> Wrote $OUT"
ls -lh "$OUT"

REL="${ROOT}/releases"
if [[ -d "$REL" ]]; then
  cp -f "$OUT" "${REL}/"
  echo "==> Copied to releases/$(basename "$OUT")"
fi
