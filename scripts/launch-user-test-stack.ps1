param(
    [switch]$WebOnly,
    [switch]$NoBuild,
    [switch]$OpenBrowser
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$runtimeDir = Join-Path $repoRoot ".runtime"
$logsDir = Join-Path $runtimeDir "logs"
$bootstrapStatePath = Join-Path $runtimeDir "user-test-bootstrap.json"
$startScriptPath = Join-Path $PSScriptRoot "start-user-test-stack.ps1"
$bootstrapStdoutPath = Join-Path $logsDir "user-test-bootstrap.stdout.log"
$bootstrapStderrPath = Join-Path $logsDir "user-test-bootstrap.stderr.log"

New-Item -ItemType Directory -Path $runtimeDir -Force | Out-Null
New-Item -ItemType Directory -Path $logsDir -Force | Out-Null

if (Test-Path $bootstrapStatePath) {
    $existingBootstrap = Get-Content $bootstrapStatePath | ConvertFrom-Json
    if ($existingBootstrap.pid -and (Get-Process -Id $existingBootstrap.pid -ErrorAction SilentlyContinue)) {
        Write-Host "User-test stack startup is already in progress." -ForegroundColor Yellow
        Write-Host "Logs:"
        Write-Host "  bootstrap: $($existingBootstrap.stdout)"
        Write-Host "  bootstrap: $($existingBootstrap.stderr)"
        Write-Host "Stop it with: bun run user-test:stop"
        exit 0
    }

    Remove-Item $bootstrapStatePath -Force -ErrorAction SilentlyContinue
}

$argumentList = @(
    "-NoLogo",
    "-NoProfile",
    "-NonInteractive",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    $startScriptPath
)

if ($WebOnly) {
    $argumentList += "-WebOnly"
}

if ($NoBuild) {
    $argumentList += "-NoBuild"
}

if ($OpenBrowser) {
    $argumentList += "-OpenBrowser"
}

$bootstrapProcess = Start-Process `
    -FilePath "powershell.exe" `
    -ArgumentList $argumentList `
    -WorkingDirectory $repoRoot `
    -PassThru `
    -RedirectStandardOutput $bootstrapStdoutPath `
    -RedirectStandardError $bootstrapStderrPath `
    -WindowStyle Hidden

$bootstrapState = [pscustomobject]@{
    pid = $bootstrapProcess.Id
    startedAt = (Get-Date).ToString("o")
    stdout = $bootstrapStdoutPath
    stderr = $bootstrapStderrPath
}

$bootstrapState | ConvertTo-Json -Depth 3 | Set-Content -Path $bootstrapStatePath

Write-Host "Starting user-test stack in the background." -ForegroundColor Green
Write-Host "Logs:"
Write-Host "  bootstrap: $bootstrapStdoutPath"
Write-Host "  bootstrap: $bootstrapStderrPath"
Write-Host "Stop it with: bun run user-test:stop"
exit 0
