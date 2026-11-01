# Packs a release from dist\huedra.exe into dist\release\<version>\:
#   huedra.exe                          the program, as the updater downloads it
#   release.toml                        version, file name, size and SHA-256
#   huedra-<version>-windows-x64.zip    the program and its documents
# With -SecretKey, signs release.toml and the zip with minisign (on PATH, or
# -Minisign), which asks for the key's password. Upload every file in the
# folder to the release address. Feature docs: docs/features/11-updates.md,
# docs/features/12-packaging.md.
param(
    [string]$Exe = (Join-Path $PSScriptRoot '..\dist\huedra.exe'),
    [string]$SecretKey,
    [string]$Minisign = 'minisign',
    [string]$OutDir
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem

$root = Split-Path -Parent $PSScriptRoot
$Exe = (Resolve-Path $Exe).Path
$line = (& $Exe --version | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $line -notmatch '^Huedra (\d+\.\d+\.\d+)$') {
    throw "huedra.exe --version printed '$line'"
}
$version = $Matches[1]
if (-not $OutDir) { $OutDir = Join-Path $root "dist\release\$version" }
New-Item -ItemType Directory -Force $OutDir | Out-Null

$program = Join-Path $OutDir 'huedra.exe'
Copy-Item $Exe $program -Force
$size = (Get-Item $program).Length
$sha = (Get-FileHash $program -Algorithm SHA256).Hash.ToLowerInvariant()

# LF line endings and no byte-order mark: the signature covers these bytes.
$manifest = "version = `"$version`"`nfile = `"huedra.exe`"`nsize = $size`nsha256 = `"$sha`"`n"
$utf8 = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText((Join-Path $OutDir 'release.toml'), $manifest, $utf8)

$zip = Join-Path $OutDir "huedra-$version-windows-x64.zip"
if (Test-Path $zip) { Remove-Item $zip }
$archive = [System.IO.Compression.ZipFile]::Open($zip, 'Create')
try {
    $null = [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $program, 'huedra.exe', 'Optimal')
    foreach ($doc in 'README.md', 'SECURITY.md', 'CHANGELOG.md', 'PRIVACY.md', 'LICENSE-MIT', 'LICENSE-APACHE') {
        $path = Join-Path $root $doc
        if (Test-Path $path) {
            $null = [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $path, $doc, 'Optimal')
        }
    }
} finally {
    $archive.Dispose()
}

if ($SecretKey) {
    & $Minisign -S -s $SecretKey -t "huedra $version" -m (Join-Path $OutDir 'release.toml') $zip
    if ($LASTEXITCODE -ne 0) { throw 'minisign could not sign the release' }
} else {
    Write-Warning 'Not signed: pass -SecretKey. The updater refuses an unsigned release.'
}
Get-ChildItem $OutDir | Select-Object Name, Length
