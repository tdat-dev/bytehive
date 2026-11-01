# Build the native ByteHive installer: a release build of crates/native-shell,
# packed with NSIS (crates/native-shell/installer/bytehive.nsi) together with
# the browser-extension folder. Prints the installer's path.
#
#   powershell -File tools/build_installer.ps1            # build + pack
#   powershell -File tools/build_installer.ps1 -NoBuild   # pack the last build
param([switch]$NoBuild)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$crate = Join-Path $root "crates\native-shell"

$version = (Select-String -Path (Join-Path $crate "Cargo.toml") -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
if (-not $NoBuild) {
  Push-Location $crate
  cargo build --release
  if ($LASTEXITCODE -ne 0) { Pop-Location; throw "cargo build failed" }
  Pop-Location
}
$exe = Join-Path $crate "target\release\native-shell.exe"
if (-not (Test-Path $exe)) { throw "no release build at $exe" }

$makensis = (Get-Command makensis -ErrorAction SilentlyContinue).Source
foreach ($c in @("${env:ProgramFiles(x86)}\NSIS\makensis.exe", "$env:ProgramFiles\NSIS\makensis.exe", "$env:LOCALAPPDATA\tauri\NSIS\makensis.exe")) {
  if (-not $makensis -and (Test-Path $c)) { $makensis = $c }
}
if (-not $makensis) { throw "makensis not found: install NSIS 3" }

$out = Join-Path $crate "target\release\bundle"
New-Item -ItemType Directory -Force $out | Out-Null
$setup = Join-Path $out "ByteHive_${version}_x64-setup.exe"
$ext = Join-Path $root "browser-extension"
& $makensis /V2 "/DVERSION=$version" "/DEXE=$exe" "/DEXT=$ext" "/DOUTFILE=$setup" (Join-Path $crate "installer\bytehive.nsi")
if ($LASTEXITCODE -ne 0) { throw "makensis failed" }
$setup
