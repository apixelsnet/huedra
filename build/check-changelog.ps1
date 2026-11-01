# Checks the changelogs:
# - CHANGELOG.md (the program): with -Version, a section "## <version>" or a
#   non-empty "## Unreleased" section, which become that release's notes;
#   without it, an "## Unreleased" section to add changes to.
# - extras/browser/CHANGELOG.md: a section for the extension's version in
#   extras/browser/manifest.json.
# Feature doc: docs/features/12-packaging.md.
param([string]$Version)
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot

# The lines under "## <heading>" up to the next "## ", or $null without one.
function Get-Section([string]$Changelog, [string]$Heading) {
    $lines = Get-Content -Encoding utf8 (Join-Path $root $Changelog)
    $pattern = '^## ' + [regex]::Escape($Heading) + '(\s|$)'
    $start = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match $pattern) { $start = $i + 1; break }
    }
    if ($start -lt 0) { return $null }
    $body = @()
    for ($i = $start; $i -lt $lines.Count -and $lines[$i] -notmatch '^## '; $i++) { $body += $lines[$i] }
    ($body -join "`n").Trim()
}

if ($Version) {
    if ($null -ne (Get-Section 'CHANGELOG.md' $Version)) {
        "CHANGELOG.md has a section for Huedra $Version"
    } elseif (Get-Section 'CHANGELOG.md' 'Unreleased') {
        "CHANGELOG.md: Huedra $Version gets the Unreleased section"
    } else {
        throw "CHANGELOG.md has neither a section '## $Version' nor anything under '## Unreleased'"
    }
} elseif ($null -eq (Get-Section 'CHANGELOG.md' 'Unreleased')) {
    throw "CHANGELOG.md has no '## Unreleased' section"
} else {
    'CHANGELOG.md has an Unreleased section'
}

$manifest = Get-Content -Raw -Encoding utf8 (Join-Path $root 'extras/browser/manifest.json') | ConvertFrom-Json
if ($null -eq (Get-Section 'extras/browser/CHANGELOG.md' $manifest.version)) {
    throw "extras/browser/CHANGELOG.md has no section '## $($manifest.version)'"
}
"extras/browser/CHANGELOG.md has a section for the extension $($manifest.version)"
