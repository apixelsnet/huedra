# Fails when a version has no changelog section: the program's version
# (Cargo.toml) in CHANGELOG.md, and the browser extension's version
# (extras/browser/manifest.json) in extras/browser/CHANGELOG.md. A section
# starts with "## <version>". The release workflow uses the program's section
# as the release notes. Feature doc: docs/features/12-packaging.md.
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot

function Assert-Section([string]$Changelog, [string]$Version, [string]$What) {
    $path = Join-Path $root $Changelog
    $pattern = '^## ' + [regex]::Escape($Version) + '(\s|$)'
    if (-not (Select-String -Path $path -Pattern $pattern -Quiet)) {
        throw "$Changelog has no section '## $Version' for $What"
    }
    "$Changelog has a section for $What $Version"
}

$cargo = Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version = "(.+)"$' | Select-Object -First 1
if (-not $cargo) { throw 'Cargo.toml has no workspace version' }
Assert-Section 'CHANGELOG.md' $cargo.Matches[0].Groups[1].Value 'Huedra'

$manifest = Get-Content -Raw -Encoding utf8 (Join-Path $root 'extras/browser/manifest.json') | ConvertFrom-Json
Assert-Section 'extras/browser/CHANGELOG.md' $manifest.version 'the browser extension'
