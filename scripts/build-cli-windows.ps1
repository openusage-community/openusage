# Windows support adapted from barramee27/crossusage (MIT): https://github.com/barramee27/crossusage
# Build a portable Windows CLI bundle: openusage-cli.exe + resources/bundled_plugins
# (same layout as scripts/build-cli-tarball.sh on Linux/macOS, but .zip for INSTALL_MODE=cli on Windows).
#
# Run from repo root in PowerShell:
#   .\scripts\build-cli-windows.ps1
#
# Writes:
#   openusage-cli_<version>_windows_<amd64|arm64>.zip  (repo root)
#   copies into releases\ for git (optional)
#
# Requires: Rust toolchain for current host (x64 or ARM64 Windows).
$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $Root

$pkg = Get-Content -Raw (Join-Path $Root "package.json") | ConvertFrom-Json
$Version = [string]$pkg.version.Trim()

$arch = $env:PROCESSOR_ARCHITECTURE
switch ($arch) {
  "AMD64" { $Tag = "amd64" }
  "ARM64" { $Tag = "arm64" }
  default { throw "Unsupported PROCESSOR_ARCHITECTURE=$arch (need AMD64 or ARM64)" }
}

Write-Host "==> Building openusage-cli (release) on Windows $Tag"
cargo build --release -p openusage-cli
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exeSrc = Join-Path $Root "target\release\openusage-cli.exe"
if (-not (Test-Path -LiteralPath $exeSrc)) {
  throw "Expected $exeSrc after cargo build"
}

$stage = Join-Path $env:TEMP ("openusage-cli-stage-" + [guid]::NewGuid().ToString())
$rootStage = Join-Path $stage "root"
New-Item -ItemType Directory -Path (Join-Path $rootStage "resources") -Force | Out-Null
Copy-Item -LiteralPath $exeSrc -Destination (Join-Path $rootStage "openusage-cli.exe") -Force
$bundled = Join-Path $Root "src-tauri\resources\bundled_plugins"
if (-not (Test-Path -LiteralPath $bundled)) {
  throw "Missing $bundled — run from full repo checkout with plugins."
}
Copy-Item -LiteralPath $bundled -Destination (Join-Path $rootStage "resources\bundled_plugins") -Recurse -Force

$zipName = "openusage-cli_${Version}_windows_${Tag}.zip"
$zipPath = Join-Path $Root $zipName
if (Test-Path -LiteralPath $zipPath) { Remove-Item -LiteralPath $zipPath -Force }
Compress-Archive -Path "$rootStage\*" -DestinationPath $zipPath -Force

Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "==> Wrote $zipPath"
Get-Item $zipPath | Select-Object Name, Length

$rel = Join-Path $Root "releases"
if (Test-Path -LiteralPath $rel) {
  Copy-Item -LiteralPath $zipPath -Destination (Join-Path $rel $zipName) -Force
  $legacy = Join-Path $rel "openusage-cli_windows_${Tag}.zip"
  Copy-Item -LiteralPath $zipPath -Destination $legacy -Force
  Write-Host "==> Copied to releases\ (and legacy openusage-cli_windows_${Tag}.zip)"
}
