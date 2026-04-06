$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$runtimeDir = Join-Path $repoRoot ".runtime"
$statePath = Join-Path $runtimeDir "dev-stack.json"
$bootstrapStatePath = Join-Path $runtimeDir "dev-bootstrap.json"

if (Test-Path $bootstrapStatePath) {
    $bootstrapState = Get-Content $bootstrapStatePath | ConvertFrom-Json
    $bootstrapProcess = Get-Process -Id $bootstrapState.pid -ErrorAction SilentlyContinue
    if ($bootstrapProcess) {
        Write-Host "Stopping dev bootstrap (PID $($bootstrapState.pid))..." -ForegroundColor Cyan
        & taskkill /PID $bootstrapState.pid /T /F | Out-Null
    }

    Remove-Item $bootstrapStatePath -Force -ErrorAction SilentlyContinue
}

if (Test-Path $statePath) {
    $state = Get-Content $statePath | ConvertFrom-Json
    foreach ($processInfo in $state.processes) {
        $process = Get-Process -Id $processInfo.pid -ErrorAction SilentlyContinue
        if ($process) {
            Write-Host "Stopping $($processInfo.name) (PID $($processInfo.pid))..." -ForegroundColor Cyan
            & taskkill /PID $processInfo.pid /T /F | Out-Null
        }
    }

    Remove-Item $statePath -Force
}

if (Get-Command docker -ErrorAction SilentlyContinue) {
    Write-Host "Stopping PostgreSQL + API..." -ForegroundColor Cyan
    & docker compose down | Out-Host
}

Write-Host "Dev stack stopped." -ForegroundColor Green
exit 0
