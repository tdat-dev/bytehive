# Build the native ByteHive installer: a release build of native/, packed with
# NSIS (native/installer/bytehive.nsi) together with the browser-extension
# folder, the app icon and the MCP server the Company's people run (mcp/, as
# one file: mcp-server\bytehive-mcp.mjs). Needs Node 22.13+ and npm. Prints the
# installer's path.
#
#   powershell -File tools/build_native_installer.ps1            # build + pack
#   powershell -File tools/build_native_installer.ps1 -NoBuild   # pack the last build
param([switch]$NoBuild)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$crate = Join-Path $root "native"

$version = (Select-String -Path (Join-Path $crate "Cargo.toml") -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
if (-not $NoBuild) {
  Push-Location $crate
  cargo build --release
  if ($LASTEXITCODE -ne 0) { Pop-Location; throw "cargo build failed" }
  Pop-Location
}
$exe = Join-Path $crate "target\release\bytehive-native.exe"
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
$icon = Join-Path $root "src-tauri\icons\icon.ico"

# The MCP server, bundled with its packages into one file that plain node runs.
$mcp = Join-Path $root "mcp"
$server = Join-Path $out "mcp-server\bytehive-mcp.mjs"
npm --prefix $mcp ci --no-audit --no-fund
if ($LASTEXITCODE -ne 0) { throw "npm ci in mcp failed" }
npm --prefix $mcp run bundle -- $server
if ($LASTEXITCODE -ne 0) { throw "bundling the MCP server failed" }
if (-not (Test-Path $server)) { throw "no MCP server at $server" }

& $makensis /V2 "/DVERSION=$version" "/DEXE=$exe" "/DEXT=$ext" "/DICON=$icon" "/DMCP=$(Split-Path -Parent $server)" "/DOUTFILE=$setup" (Join-Path $crate "installer\bytehive.nsi")
if ($LASTEXITCODE -ne 0) { throw "makensis failed" }
$setup
