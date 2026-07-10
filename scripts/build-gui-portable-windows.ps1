# Windows support adapted from barramee27/crossusage (MIT): https://github.com/barramee27/crossusage
# Portable **GUI** Windows bundle: **openusage.exe** (launcher when WebView2Loader.dll exists) +
# **openusage_gui.exe** (Tauri) + **openusage-cli.exe** + **WebView2Loader.dll** + **resources\**
# If the MSVC build does not emit WebView2Loader.dll next to the exe, we ship the Tauri binary as
# **openusage.exe** only (legacy layout).
# From Linux (GNU zip with launcher): scripts/build-gui-portable-zip-windows-gnu.sh after
#   bun run tauri build --target x86_64-pc-windows-gnu
# Run from repo root after a release GUI build, e.g.:
#   bun run tauri build
# Output: openusage_<version>_windows_<amd64|arm64>.zip (repo root)
$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $Root

$pkg = Get-Content -Raw (Join-Path $Root "package.json") | ConvertFrom-Json
$Version = [string]$pkg.version.Trim()

function Resolve-FirstPath {
  param([string[]]$Patterns)
  foreach ($pattern in $Patterns) {
    $matches = Get-ChildItem -Path (Join-Path $Root $pattern) -File -ErrorAction SilentlyContinue |
      Sort-Object FullName
    if ($matches) {
      return $matches[0].FullName
    }
  }
  return $null
}

switch ($env:PROCESSOR_ARCHITECTURE) {
  "AMD64" { $Tag = "amd64" }
  "ARM64" { $Tag = "arm64" }
  default { throw "Unsupported PROCESSOR_ARCHITECTURE=$($env:PROCESSOR_ARCHITECTURE) (need AMD64 or ARM64)" }
}

$gui = Resolve-FirstPath @("target\release\openusage.exe", "target\*\release\openusage.exe")
$cli = Resolve-FirstPath @("target\release\openusage-cli.exe", "target\*\release\openusage-cli.exe")
$wv2 = Resolve-FirstPath @("target\release\WebView2Loader.dll", "target\*\release\WebView2Loader.dll")
$res = Join-Path $Root "src-tauri\resources"
$icons = Join-Path $Root "src-tauri\icons"

if (-not $gui -or -not (Test-Path -LiteralPath $gui)) {
  throw "Missing openusage.exe under target\release or target\<triple>\release — run: bun run tauri build"
}
if (-not $cli -or -not (Test-Path -LiteralPath $cli)) {
  throw "Missing openusage-cli.exe under target\release or target\<triple>\release — run: bun run tauri build (or cargo build --release -p openusage-cli)"
}
if (-not (Test-Path -LiteralPath (Join-Path $res "bundled_plugins"))) {
  throw "Missing bundled_plugins — run: bun run bundle:plugins"
}

$stage = Join-Path $env:TEMP ("openusage-gui-portable-" + [guid]::NewGuid().ToString())
$rootStage = Join-Path $stage "root"
New-Item -ItemType Directory -Path $rootStage -Force | Out-Null

Copy-Item -LiteralPath $cli -Destination (Join-Path $rootStage "openusage-cli.exe") -Force
Copy-Item -LiteralPath $res -Destination (Join-Path $rootStage "resources") -Recurse -Force
Copy-Item -LiteralPath $icons -Destination (Join-Path $rootStage "icons") -Recurse -Force
Copy-Item -LiteralPath (Join-Path $Root "src-tauri\resources\WINDOWS-PORTABLE.txt") -Destination (Join-Path $rootStage "README-Windows.txt") -Force

if ($wv2 -and (Test-Path -LiteralPath $wv2)) {
  Write-Host "==> Building openusage-win-launcher (WebView2Loader.dll present) …"
  cargo build --release -p openusage-win-launcher
  $launcher = Join-Path $Root "target\release\openusage-win-launcher.exe"
  if (-not (Test-Path -LiteralPath $launcher)) {
    throw "Missing $launcher after cargo build -p openusage-win-launcher"
  }
  Copy-Item -LiteralPath $gui -Destination (Join-Path $rootStage "openusage_gui.exe") -Force
  Copy-Item -LiteralPath $launcher -Destination (Join-Path $rootStage "openusage.exe") -Force
  Copy-Item -LiteralPath $wv2 -Destination (Join-Path $rootStage "WebView2Loader.dll") -Force
} else {
  Write-Warning "Missing $wv2 — shipping single openusage.exe (MSVC layout; no embedded WebView2 loader stub)."
  Copy-Item -LiteralPath $gui -Destination (Join-Path $rootStage "openusage.exe") -Force
}

$zipName = "openusage_${Version}_windows_${Tag}.zip"
$zipPath = Join-Path $Root $zipName
if (Test-Path -LiteralPath $zipPath) { Remove-Item -LiteralPath $zipPath -Force }
Compress-Archive -Path (Join-Path $rootStage "*") -DestinationPath $zipPath -Force
Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "==> Wrote $zipPath"
Get-Item $zipPath | Select-Object Name, Length

$rel = Join-Path $Root "releases"
if (Test-Path -LiteralPath $rel) {
  Copy-Item -LiteralPath $zipPath -Destination (Join-Path $rel $zipName) -Force
  Write-Host "==> Copied to releases\"
}
