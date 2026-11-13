$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$tag = 'v0.42.0'
$asset = "libmpv-$tag-x86_64-w64-mingw32-lgpl.zip"
$expectedSha256 = '8e5f77430efc53ef175804ba6618527e3af0448a0dcabf04d3cc72659d1a6fce'
$url = "https://github.com/mpv-player/mpv/releases/download/$tag/$asset"
$downloadDir = Join-Path $env:TEMP 'openwallpaper-mpv'
$archive = Join-Path $downloadDir $asset
$extractDir = Join-Path $downloadDir 'extract'
$target = Join-Path $repoRoot 'src-tauri/resources/mpv-runtime'

New-Item -ItemType Directory -Force -Path $downloadDir | Out-Null
if (Test-Path -LiteralPath $archive) {
    $cachedHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($cachedHash -ne $expectedSha256) { Remove-Item -LiteralPath $archive -Force }
}
if (-not (Test-Path -LiteralPath $archive)) {
    Write-Host "Downloading official mpv $tag LGPL libmpv runtime..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri $url -OutFile $archive
}
$actualHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualHash -ne $expectedSha256) {
    throw "libmpv archive SHA-256 mismatch. Expected $expectedSha256, got $actualHash."
}

if (Test-Path $extractDir) { Remove-Item $extractDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $extractDir | Out-Null
Expand-Archive -LiteralPath $archive -DestinationPath $extractDir -Force

$mpvDll = Get-ChildItem -Path $extractDir -Recurse -File -Filter 'libmpv-2.dll' |
    Select-Object -First 1
if (-not $mpvDll) { throw 'The official LGPL archive does not contain libmpv-2.dll.' }

$runtimeDlls = @(Get-ChildItem -Path $extractDir -Recurse -File -Filter '*.dll')
if ($runtimeDlls.Count -eq 0) { throw 'The libmpv archive contains no runtime DLL dependencies.' }

if (Test-Path $target) { Remove-Item $target -Recurse -Force }
New-Item -ItemType Directory -Force -Path $target | Out-Null

# Windows dependency DLL lookup is restricted to this directory at runtime.
# Flatten DLLs into one directory and reject conflicting duplicate basenames.
foreach ($dll in $runtimeDlls) {
    $destination = Join-Path $target $dll.Name
    if (Test-Path -LiteralPath $destination) {
        $existingHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
        $incomingHash = (Get-FileHash -LiteralPath $dll.FullName -Algorithm SHA256).Hash
        if ($existingHash -ne $incomingHash) {
            throw "Conflicting runtime DLL name '$($dll.Name)' in upstream archive."
        }
        continue
    }
    Copy-Item -LiteralPath $dll.FullName -Destination $destination -Force
}
if (-not (Test-Path -LiteralPath (Join-Path $target 'libmpv-2.dll'))) {
    throw 'libmpv-2.dll was not staged.'
}

# Keep upstream licence/readme material and its directory structure.
$licenseRoot = Join-Path $target 'licenses'
$noticeFiles = Get-ChildItem -Path $extractDir -Recurse -File | Where-Object {
    $_.Name -match '^(LICENSE|COPYING|COPYRIGHT|NOTICE|AUTHORS|README)([._-].*)?$' -or
    $_.Extension -eq '.txt'
}
foreach ($file in $noticeFiles) {
    $relative = $file.FullName.Substring($extractDir.Length).TrimStart('\', '/')
    $destination = Join-Path $licenseRoot $relative
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $destination) | Out-Null
    Copy-Item -LiteralPath $file.FullName -Destination $destination -Force
}

@"
OpenWallpaper bundled media runtime
Project: mpv
Release: $tag
Official upstream release: https://github.com/mpv-player/mpv/releases/tag/$tag
Asset: $asset
SHA-256: $expectedSha256
Architecture: x86_64 Windows / MinGW
Licence mode: LGPL build, as identified by the upstream release
The upstream release notes state that the LGPL build excludes GPL-only mpv and FFmpeg components.
See licenses/ and the upstream Copyright file for notices.
"@ | Set-Content -LiteralPath (Join-Path $target 'RUNTIME-SOURCE.txt') -Encoding UTF8

$totalBytes = (Get-ChildItem $target -Recurse -File | Measure-Object -Property Length -Sum).Sum
Write-Host ("Bundled {0} DLL files ({1:N1} MB) into {2}" -f $runtimeDlls.Count, ($totalBytes / 1MB), $target) -ForegroundColor Green
