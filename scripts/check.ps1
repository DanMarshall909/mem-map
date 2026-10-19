param([ValidateSet('fast', 'slow', 'all')][string]$Lane = 'all')
$ErrorActionPreference = 'Stop'
$driver = Join-Path $PSScriptRoot 'cargo.ps1'
if ($Lane -in 'fast', 'all') {
    & $driver test --lib --bin mem-map
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
}
if ($Lane -in 'slow', 'all') {
    & $driver -CargoArgs @('test', '--test', 'windows_live', '--', '--ignored')
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
}
