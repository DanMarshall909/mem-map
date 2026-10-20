param([string]$OutputDirectory, [string]$Executable)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
if (!$Executable) { $Executable = Join-Path $projectRoot 'MemMap.exe' }
if (!$OutputDirectory) {
    $OutputDirectory = Join-Path $env:LOCALAPPDATA ('mem-map-build\evidence\tauri-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
}
& node (Join-Path $PSScriptRoot 'desktop-smoke.mjs') $Executable $OutputDirectory
exit $LASTEXITCODE
