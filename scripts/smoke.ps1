param([string]$OutputDirectory, [string]$BackendExecutable)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
if (!$OutputDirectory) {
    $OutputDirectory = Join-Path $env:LOCALAPPDATA ('mem-map-build\evidence\headless-' + (Get-Date -Format 'yyyyMMdd-HHmmss-fff'))
}
if (!$BackendExecutable) {
    & (Join-Path $PSScriptRoot 'cargo.ps1') build --example headless-test-host --no-default-features --locked
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
    $metadata = & (Join-Path $PSScriptRoot 'cargo.ps1') metadata --no-deps --format-version 1 --no-default-features --locked
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
    $targetDirectory = ($metadata | ConvertFrom-Json).target_directory
    $BackendExecutable = Join-Path $targetDirectory 'debug\examples\headless-test-host.exe'
}
& npm.cmd --prefix (Join-Path $projectRoot 'frontend') run build
if ($LASTEXITCODE) { exit $LASTEXITCODE }
& node (Join-Path $PSScriptRoot 'headless-smoke.mjs') $BackendExecutable $OutputDirectory
exit $LASTEXITCODE
