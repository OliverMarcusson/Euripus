$ErrorActionPreference = "Stop"

$devStopScript = Join-Path $PSScriptRoot "dev-stop.ps1"
$devStartScript = Join-Path $PSScriptRoot "dev-start.ps1"

function Resolve-PowerShellExecutable {
    if (Get-Command pwsh -ErrorAction SilentlyContinue) {
        return "pwsh"
    }

    if (Get-Command powershell -ErrorAction SilentlyContinue) {
        return "powershell"
    }

    throw "Required command 'pwsh' or 'powershell' was not found on PATH."
}

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    throw "Required command 'docker' was not found on PATH."
}

$powerShellExecutable = Resolve-PowerShellExecutable

Write-Host "Stopping existing dev stack..." -ForegroundColor Cyan
& $powerShellExecutable -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $devStopScript
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "Removing local PostgreSQL and Meilisearch volumes..." -ForegroundColor Cyan
& docker compose down -v | Out-Host

Write-Host "Rebuilding and restarting dev stack..." -ForegroundColor Cyan
& $powerShellExecutable -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $devStartScript

exit $LASTEXITCODE
