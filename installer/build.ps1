$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

function Require-Command([string]$Name, [string]$InstallHint) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "$Name was not found. $InstallHint"
    }
}

Require-Command 'cargo' 'Install Rust from https://rustup.rs/ and add the x86_64-pc-windows-msvc target.'
Require-Command 'rustc' 'Install Rust from https://rustup.rs/.'
Require-Command 'npm' 'Install Node.js LTS from https://nodejs.org/.'

$target = 'x86_64-pc-windows-msvc'
rustup target add $target

Write-Host 'Installing npm dependencies...' -ForegroundColor Cyan
npm install
if ($LASTEXITCODE -ne 0) { throw 'npm install failed.' }

Write-Host 'Building frontend...' -ForegroundColor Cyan
npm run build
if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed.' }

Write-Host 'Running Rust tests...' -ForegroundColor Cyan
Push-Location 'src-tauri'
cargo test --target $target
$testExit = $LASTEXITCODE
Pop-Location
if ($testExit -ne 0) { throw 'Rust tests failed.' }

Write-Host 'Building NSIS and MSI installers...' -ForegroundColor Cyan
npm run tauri build -- --bundles nsis,msi
if ($LASTEXITCODE -ne 0) { throw 'Tauri packaging failed.' }

$bundleRoot = Join-Path $PWD 'src-tauri/target/release/bundle'
$installers = Get-ChildItem $bundleRoot -Recurse -File -Include *.exe,*.msi
if (-not $installers) { throw "No installers were produced under $bundleRoot" }

Write-Host "`nInstallers created:" -ForegroundColor Green
$installers | Select-Object -ExpandProperty FullName
Write-Host "`nNote: media playback requires the separately obtained libmpv runtime (mpv-2.dll). HTML wallpapers do not require it." -ForegroundColor Yellow
