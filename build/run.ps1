# Stops any running Huedra, then starts dist\huedra.exe. Arguments pass through.
$exe = Join-Path $PSScriptRoot '..\dist\huedra.exe'
if (-not (Test-Path $exe)) { throw 'dist\huedra.exe not found. Run build/remote-build.sh first.' }
Get-Process -Name huedra -ErrorAction SilentlyContinue | Stop-Process -Force
& $exe @args
exit $LASTEXITCODE
