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
        # LLD correctly combines Rust's native and generated raw-dylib imports.
        # Bundled GNU BFD can leave live imports after an early null thunk,
        # producing a successful link followed by a crash before Rust main.
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $llvm.FullName 'bin\x86_64-w64-mingw32-clang.exe'
        # An external clang driver disables rustc's automatic GNU-runtime
        # fallback. Keep Rust's matching startup objects, libgcc and unwind
        # libraries instead of mixing them with clang's UCRT defaults.
        # Preserve caller flags and avoid appending our requirement twice.
        if ($env:CARGO_ENCODED_RUSTFLAGS) {
            $linkFlags = [char]31 + '-C' + [char]31 + 'link-self-contained=yes'
            if (!$env:CARGO_ENCODED_RUSTFLAGS.EndsWith($linkFlags)) {
                $env:CARGO_ENCODED_RUSTFLAGS += $linkFlags
            }
        } elseif ($env:RUSTFLAGS -notmatch '(^|\s)-C\s*link-self-contained=yes\s*$') {
            $env:RUSTFLAGS = if ($env:RUSTFLAGS) { $env:RUSTFLAGS + ' -C link-self-contained=yes' } else { '-C link-self-contained=yes' }
        }
    }
    $cargoExe = Join-Path $env:CARGO_HOME 'bin\cargo.exe'
} else { $cargoExe = 'cargo' }
Push-Location (Split-Path $PSScriptRoot -Parent)
try { & $cargoExe @CargoArgs; $result = $LASTEXITCODE } finally { Pop-Location }
exit $result
