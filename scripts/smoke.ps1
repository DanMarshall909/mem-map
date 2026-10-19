param([string]$OutputDirectory = 'artifacts\smoke')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$outputPath = [IO.Path]::GetFullPath((Join-Path $projectRoot $OutputDirectory))
New-Item -ItemType Directory -Path $outputPath -Force | Out-Null
Set-Content -LiteralPath (Join-Path $outputPath 'smoke.txt') -Value 'result=RUNNING'
$process = Start-Process -FilePath (Join-Path $projectRoot 'target\release\mem-map.exe') -ArgumentList @('--smoke-test', ('"' + $outputPath + '"')) -WindowStyle Hidden -PassThru
if (!$process.WaitForExit(45000)) {
    $process.Kill()
    throw 'The task-owned smoke process timed out after 45 seconds.'
}
if ($process.ExitCode -ne 0) { throw "Smoke process exited $($process.ExitCode)" }
$report = Get-Content -LiteralPath (Join-Path $outputPath 'smoke.txt') -Raw
if ($report -notmatch '(?m)^result=PASS\r?$') { throw $report }
$report
