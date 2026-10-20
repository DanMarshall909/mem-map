param([ValidatePattern('^[a-zA-Z0-9._-]+\.exe$')][string]$OutputName = 'MemMap.exe')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Push-Location (Join-Path $projectRoot 'frontend')
try { & npm.cmd run build; $frontendResult = $LASTEXITCODE } finally { Pop-Location }
if ($frontendResult) { exit $frontendResult }
& (Join-Path $PSScriptRoot 'cargo.ps1') build --release --locked
if ($LASTEXITCODE) { exit $LASTEXITCODE }
$targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } elseif (Test-Path (Join-Path $env:LOCALAPPDATA 'mem-map-build\cargo\bin\cargo.exe')) { Join-Path $env:LOCALAPPDATA 'mem-map-build\targets\tauri' } else { Join-Path $projectRoot 'target' }
Copy-Item -LiteralPath (Join-Path $targetRoot 'release\mem-map.exe') -Destination (Join-Path $projectRoot $OutputName)
# GNU uses WebView2's supported dynamic loader; keep it beside the executable.
$loaderFiles = @(Get-ChildItem -Path (Join-Path $targetRoot 'release\build\webview2-com-sys-*\out\x64\WebView2Loader.dll') -ErrorAction SilentlyContinue)
if ($loaderFiles.Count) {
    $loaderHashes = @($loaderFiles | Get-FileHash | Select-Object -ExpandProperty Hash -Unique)
    if ($loaderHashes.Count -ne 1) { throw 'Ambiguous WebView2 loader versions in build output.' }
    Copy-Item -LiteralPath $loaderFiles[0].FullName -Destination (Join-Path $projectRoot 'WebView2Loader.dll')
}
Get-Item -LiteralPath (Join-Path $projectRoot $OutputName) | Select-Object FullName, Length
