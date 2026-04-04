$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$runtimeDir = Join-Path $repoRoot ".runtime"
$statePath = Join-Path $runtimeDir "user-test-stack.json"

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

Write-Host "User-test stack stopped." -ForegroundColor Green
