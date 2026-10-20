param([ValidateSet('fast', 'slow', 'all')][string]$Lane = 'all')
$ErrorActionPreference = 'Stop'
$driver = Join-Path $PSScriptRoot 'cargo.ps1'
if ($Lane -in 'fast', 'all') {
    & $driver test --lib --no-default-features --locked
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
    & $driver test --bin mem-map --locked
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
    Push-Location (Join-Path (Split-Path $PSScriptRoot -Parent) 'frontend')
    try { & npm.cmd run test:fast; $frontendResult = $LASTEXITCODE } finally { Pop-Location }
    if ($frontendResult) { exit $frontendResult }
}
if ($Lane -in 'slow', 'all') {
    & $driver -CargoArgs @('test', '--no-default-features', '--locked', '--test', 'windows_live', '--', '--ignored')
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
    & (Join-Path $PSScriptRoot 'smoke.ps1')
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
}
