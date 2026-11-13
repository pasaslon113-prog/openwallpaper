$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$tag = '20261111'
$asset = "mpv-x86_64-$tag-git-bcde6b93cc.7z"
$url = "https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/$tag/$asset"
$downloadDir = Join-Path $env:TEMP 'openwallpaper-mpv'
$archive = Join-Path $downloadDir $asset
$extractDir = Join-Path $downloadDir 'extract'
$target = Join-Path $repoRoot 'src-tauri/resources/mpv-runtime'
$sevenZip = Get-Command '7z.exe' -ErrorAction SilentlyContinue
if ($sevenZip) {
    $sevenZipPath = $sevenZip.Source
} else {
    $sevenZipPath = Join-Path $downloadDir '7zr.exe'
    New-Item -ItemType Directory -Force -Path $downloadDir | Out-Null
    Invoke-WebRequest -Uri 'https://www.7-zip.org/a/7zr.exe' -OutFile $sevenZipPath
}
New-Item -ItemType Directory -Force -Path $downloadDir | Out-Null
if (-not (Test-Path $archive)) {
    Write-Host "Downloading pinned Windows libmpv runtime: $tag" -ForegroundColor Cyan
    Invoke-WebRequest -Uri $url -OutFile $archive
}
if (Test-Path $extractDir) { Remove-Item $extractDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $extractDir | Out-Null
& $sevenZipPath x $archive "-o$extractDir" -y | Out-Host
if ($LASTEXITCODE -ne 0) { throw 'Could not extract the official mpv runtime archive.' }
$mpvDll = Get-ChildItem $extractDir -Recurse -File -Filter 'mpv-2.dll' | Select-Object -First 1
if (-not $mpvDll) { throw 'The downloaded mpv archive does not contain mpv-2.dll.' }
if (Test-Path $target) { Remove-Item $target -Recurse -Force }
New-Item -ItemType Directory -Force -Path $target | Out-Null
# Keep all runtime DLLs together: Windows resolves mpv's transitive DLL dependencies
# from the directory containing mpv-2.dll.
Get-ChildItem $mpvDll.DirectoryName -File -Filter '*.dll' | Copy-Item -Destination $target -Force
Get-ChildItem $extractDir -Recurse -File | Where-Object {
    $_.Name -match '^(LICENSE|COPYING|README|AUTHORS)(\..*)?$' -or $_.Extension -eq '.txt'
} | Copy-Item -Destination $target -Force
if (-not (Test-Path (Join-Path $target 'mpv-2.dll'))) { throw 'mpv-2.dll was not staged.' }
Write-Host "Bundled libmpv runtime staged in $target" -ForegroundColor Green
Get-ChildItem $target -File | Measure-Object -Property Length -Sum | ForEach-Object {
    Write-Host ("Runtime files: {0}; total size: {1:N1} MB" -f $_.Count, ($_.Sum / 1MB))
}
