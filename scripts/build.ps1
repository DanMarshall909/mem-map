param([ValidatePattern('^[a-zA-Z0-9._-]+\.exe$')][string]$OutputName = 'MemMap.exe')
$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'cargo.ps1') build --release --locked
if ($LASTEXITCODE) { exit $LASTEXITCODE }
$projectRoot = Split-Path $PSScriptRoot -Parent
Copy-Item -LiteralPath (Join-Path $projectRoot 'target\release\mem-map.exe') -Destination (Join-Path $projectRoot $OutputName)
Get-Item -LiteralPath (Join-Path $projectRoot $OutputName) | Select-Object FullName, Length
