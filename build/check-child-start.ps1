# Regression check: Huedra started with CREATE_NO_WINDOW releases the hidden
# console it was given, and must still be able to start programs. Before
# 0.1.4 the standard handles kept pointing at the released console, so
# every program started afterwards failed: Check for updates and Download
# and install in a window opened by the tray, and `huedra launch`.
#
# Starts `huedra launch -- cmd.exe /c exit 7` with no window, as the tray
# starts the settings window, and expects exit code 7 (launch passes on the
# program's code). A portable copy in a temporary folder is used, in Always
# SDR mode, so HDR is never touched; the tray it starts is closed afterwards.
# Refuses to run while another Huedra runs, because that one would answer.
#
#   build/check-child-start.ps1 -Exe dist\huedra.exe
param([string]$Exe = 'dist\huedra.exe')
$ErrorActionPreference = 'Stop'

if (Get-Process huedra -ErrorAction SilentlyContinue) {
    throw 'Huedra is running; close it first, this check starts its own copy.'
}
$dir = Join-Path ([IO.Path]::GetTempPath()) "huedra-child-start-$PID"
New-Item -ItemType Directory -Force (Join-Path $dir 'data') | Out-Null
$copy = Join-Path $dir 'huedra.exe'
try {
    Copy-Item $Exe $copy
    New-Item -ItemType File (Join-Path $dir 'portable') | Out-Null
    [IO.File]::WriteAllText((Join-Path $dir 'data\config.toml'),
        "mode = `"always-sdr`"`n[general]`ndisclaimer_accepted = true`n")

    $start = New-Object Diagnostics.ProcessStartInfo $copy, 'launch -- cmd.exe /c "exit 7"'
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $launch = [Diagnostics.Process]::Start($start)
    if (-not $launch.WaitForExit(60000)) {
        $launch.Kill()
        throw 'huedra launch did not end within 60 s'
    }
    if ($launch.ExitCode -ne 7) {
        $log = Get-Content (Join-Path $dir 'data\huedra.log') -ErrorAction SilentlyContinue
        throw "huedra launch exited with $($launch.ExitCode), expected 7: the program was not started. Log:`n$($log -join "`n")"
    }
    'huedra launch started with no window passed on exit code 7.'
} finally {
    & $copy exit 2>&1 | Out-Null
    for ($i = 0; $i -lt 40 -and (Get-Process huedra -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -eq $copy }); $i++) {
        Start-Sleep -Milliseconds 250
    }
    Get-Process huedra -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $copy } | Stop-Process -Force
    Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
    $global:LASTEXITCODE = 0
}
