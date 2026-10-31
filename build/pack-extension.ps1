# Packs the browser extension (extras/browser) into
# dist/huedra-extension-<version>.zip, the file uploaded to the Chrome Web
# Store, Edge Add-ons and Firefox AMO. The manifest sits at the top of the
# zip and entry names use forward slashes, as the stores require.
# Feature doc: docs/features/05-video.md.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem

$root = Split-Path -Parent $PSScriptRoot
$source = Join-Path $root 'extras\browser'
$manifest = Get-Content -Raw -Encoding utf8 (Join-Path $source 'manifest.json') | ConvertFrom-Json

# Every file the manifest names must exist; nothing else is packed.
$files = @('manifest.json')
$files += $manifest.icons.PSObject.Properties | ForEach-Object { $_.Value }
$files += $manifest.content_scripts | ForEach-Object { $_.js }
$files = $files | Select-Object -Unique
foreach ($file in $files) {
    if (-not (Test-Path (Join-Path $source $file))) { throw "manifest names a missing file: $file" }
}

$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
$zip = Join-Path $dist "huedra-extension-$($manifest.version).zip"
if (Test-Path $zip) { Remove-Item $zip }

$archive = [System.IO.Compression.ZipFile]::Open($zip, 'Create')
try {
    foreach ($file in $files) {
        $null = [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
            $archive, (Join-Path $source $file), $file.Replace('\', '/'), 'Optimal')
    }
} finally {
    $archive.Dispose()
}
Get-Item $zip | Select-Object Name, Length
$files
