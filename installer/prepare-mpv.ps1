$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$tag = '20261111'
$asset = "mpv-dev-x86_64-$tag-git-bcde6b93cc.7z"
$expectedSha256 = '273c079b11ea5575822f972756f4c9a71149a5c10130353abeb8586738938452'
$url = "https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/$tag/$asset"
$downloadDir = Join-Path $env:TEMP 'openwallpaper-mpv'
$archive = Join-Path $downloadDir $asset
$extractDir = Join-Path $downloadDir 'extract'
$target = Join-Path $repoRoot 'src-tauri/resources/mpv-runtime'

New-Item -ItemType Directory -Force -Path $downloadDir | Out-Null
$sevenZip = Get-Command '7z.exe' -ErrorAction SilentlyContinue
if ($sevenZip) {
    $sevenZipPath = $sevenZip.Source
} else {
    $sevenZipPath = Join-Path $downloadDir '7zr.exe'
    if (-not (Test-Path $sevenZipPath)) {
        Invoke-WebRequest -Uri 'https://www.7-zip.org/a/7zr.exe' -OutFile $sevenZipPath
    }
}

if (Test-Path $archive) {
    $cachedHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($cachedHash -ne $expectedSha256) { Remove-Item -LiteralPath $archive -Force }
}
if (-not (Test-Path $archive)) {
    Write-Host "Downloading pinned libmpv runtime: $tag" -ForegroundColor Cyan
    Invoke-WebRequest -Uri $url -OutFile $archive
}
$actualHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualHash -ne $expectedSha256) {
    throw "libmpv archive SHA-256 mismatch. Expected $expectedSha256, got $actualHash."
}

if (Test-Path $extractDir) { Remove-Item $extractDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $extractDir | Out-Null
& $sevenZipPath x $archive "-o$extractDir" -y | Out-Host
if ($LASTEXITCODE -ne 0) { throw 'Could not extract the libmpv runtime archive.' }

$mpvDll = Get-ChildItem $extractDir -Recurse -File |
    Where-Object { $_.Name -in @('libmpv-2.dll', 'mpv-2.dll') } |
    Select-Object -First 1
if (-not $mpvDll) { throw 'The mpv-dev archive does not contain libmpv-2.dll or mpv-2.dll.' }

$runtimeDlls = @(Get-ChildItem $extractDir -Recurse -File -Filter '*.dll')
if ($runtimeDlls.Count -eq 0) { throw 'The mpv-dev archive contains no runtime DLL dependencies.' }
$duplicateDlls = @($runtimeDlls | Group-Object Name | Where-Object Count -gt 1)
if ($duplicateDlls.Count -gt 0) {
    throw "The mpv-dev archive contains duplicate DLL names: $($duplicateDlls.Name -join ', ')"
}

if (Test-Path $target) { Remove-Item $target -Recurse -Force }
New-Item -ItemType Directory -Force -Path $target | Out-Null

# The DLL and its dependencies live together in the bundled resource directory.
foreach ($dll in $runtimeDlls) {
    Copy-Item -LiteralPath $dll.FullName -Destination (Join-Path $target $dll.Name) -Force
}
Get-ChildItem $extractDir -Recurse -File | Where-Object {
    $_.Name -match '^(LICENSE|COPYING|README|AUTHORS)(\..*)?$' -or $_.Extension -eq '.txt'
} | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $target $_.Name) -Force
}

$stagedLibrary = Join-Path $target $mpvDll.Name
if (-not (Test-Path $stagedLibrary)) { throw 'The libmpv library was not staged.' }
@"
OpenWallpaper bundled libmpv runtime
Source: https://github.com/shinchiro/mpv-winbuild-cmake
Release: $tag
Archive: $asset
SHA-256: $expectedSha256
Library: $($mpvDll.Name)
Architecture: Windows x64 baseline (not x86_64-v3)
"@ | Set-Content -LiteralPath (Join-Path $target 'RUNTIME-SOURCE.txt') -Encoding UTF8

$size = (Get-ChildItem $target -Recurse -File | Measure-Object -Property Length -Sum).Sum
Write-Host ("Bundled {0} DLL files ({1:N1} MB) into {2}" -f $runtimeDlls.Count, ($size / 1MB), $target) -ForegroundColor Green
