param([Parameter(ValueFromRemainingArguments = $true)][string[]]$CargoArgs)
$ErrorActionPreference = 'Stop'
$localTools = Join-Path $env:LOCALAPPDATA 'mem-map-build'
if (Test-Path -LiteralPath (Join-Path $localTools 'cargo\bin\cargo.exe')) {
    $env:CARGO_HOME = Join-Path $localTools 'cargo'
    $env:RUSTUP_HOME = Join-Path $localTools 'rustup'
    $env:PATH = (Join-Path $env:CARGO_HOME 'bin') + ';' + $env:PATH
    $bundledGnu = Join-Path $env:RUSTUP_HOME 'toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained'
    if (Test-Path -LiteralPath $bundledGnu) { $env:PATH = $bundledGnu + ';' + $env:PATH }
    $llvm = Get-ChildItem -LiteralPath $localTools -Directory -Filter 'llvm-mingw-*-ucrt-x86_64' | Sort-Object Name -Descending | Select-Object -First 1
    if ($llvm) {
        $env:PATH = (Join-Path $llvm.FullName 'bin') + ';' + $env:PATH
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $bundledGnu 'x86_64-w64-mingw32-gcc.exe'
    }
    $cargoExe = Join-Path $env:CARGO_HOME 'bin\cargo.exe'
} else { $cargoExe = 'cargo' }
Push-Location (Split-Path $PSScriptRoot -Parent)
try { & $cargoExe @CargoArgs; $result = $LASTEXITCODE } finally { Pop-Location }
exit $result
