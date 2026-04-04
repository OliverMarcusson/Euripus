param(
    [switch]$WebOnly,
    [switch]$NoBuild,
    [switch]$OpenBrowser
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$runtimeDir = Join-Path $repoRoot ".runtime"
$logsDir = Join-Path $runtimeDir "logs"
$statePath = Join-Path $runtimeDir "user-test-stack.json"
$bootstrapStatePath = Join-Path $runtimeDir "user-test-bootstrap.json"

function Assert-CommandAvailable {
    param([string]$CommandName)

    if (-not (Get-Command $CommandName -ErrorAction SilentlyContinue)) {
        throw "Required command '$CommandName' was not found on PATH."
    }
}

function Resolve-CommandPath {
    param([string]$CommandName)

    $command = Get-Command $CommandName -ErrorAction SilentlyContinue
    if (-not $command) {
        throw "Required command '$CommandName' was not found on PATH."
    }

    return $command.Source
}

function Wait-ForHttpEndpoint {
    param(
        [string]$Name,
        [string]$Url,
        [int]$TimeoutSeconds = 120
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 5 | Out-Null
            return
        } catch {
            Start-Sleep -Milliseconds 750
        }
    }

    throw "$Name did not become ready at $Url within $TimeoutSeconds seconds."
}

function Test-TrackedProcessRunning {
    param([pscustomobject]$ProcessInfo)

    if (-not $ProcessInfo) {
        return $false
    }

    return $null -ne (Get-Process -Id $ProcessInfo.pid -ErrorAction SilentlyContinue)
}

function Start-TrackedProcess {
    param(
        [string]$Name,
        [string]$FilePath,
        [string[]]$ArgumentList,
        [string]$WorkingDirectory
    )

    $stdoutPath = Join-Path $logsDir "$Name.stdout.log"
    $stderrPath = Join-Path $logsDir "$Name.stderr.log"
    $resolvedFilePath = Resolve-CommandPath $FilePath

    $process = Start-Process `
        -FilePath $resolvedFilePath `
        -ArgumentList $ArgumentList `
        -WorkingDirectory $WorkingDirectory `
        -PassThru `
        -RedirectStandardOutput $stdoutPath `
        -RedirectStandardError $stderrPath `
        -WindowStyle Hidden

    return [pscustomobject]@{
        name = $Name
        pid = $process.Id
        stdout = $stdoutPath
        stderr = $stderrPath
    }
}

try {
    New-Item -ItemType Directory -Path $runtimeDir -Force | Out-Null
    New-Item -ItemType Directory -Path $logsDir -Force | Out-Null

    Assert-CommandAvailable "docker"
    Assert-CommandAvailable "bun"

    if (-not (Test-Path (Join-Path $repoRoot "apps/server/.env"))) {
        throw "Missing apps/server/.env. Copy apps/server/.env.example before starting the user-test stack."
    }

    if (Test-Path $statePath) {
        $existingState = Get-Content $statePath | ConvertFrom-Json
        $desktopProcess = $existingState.processes | Where-Object { $_.name -eq "desktop" -or $_.name -eq "web" } | Select-Object -First 1
        if (Test-TrackedProcessRunning $desktopProcess) {
            Write-Host "User-test stack is already running." -ForegroundColor Yellow
            Write-Host "Stop it first with: bun run user-test:stop"
            exit 0
        }
    }

    $composeArgs = @("compose", "up")
    if (-not $NoBuild) {
        $composeArgs += "--build"
    }
    $composeArgs += @("-d", "postgres", "server")

    Write-Host "Starting PostgreSQL + API..." -ForegroundColor Cyan
    & docker @composeArgs | Out-Host

    Write-Host "Waiting for API health..." -ForegroundColor Cyan
    Wait-ForHttpEndpoint -Name "API" -Url "http://127.0.0.1:8080/health" -TimeoutSeconds 180

    Write-Host "Starting desktop web client..." -ForegroundColor Cyan
    $webProcess = Start-TrackedProcess `
        -Name "web" `
        -FilePath "bun" `
        -ArgumentList @("--cwd", "apps/desktop", "dev", "--host", "127.0.0.1") `
        -WorkingDirectory $repoRoot

    Write-Host "Waiting for frontend dev server..." -ForegroundColor Cyan
    Wait-ForHttpEndpoint -Name "Frontend" -Url "http://127.0.0.1:5173" -TimeoutSeconds 180

    if ($WebOnly) {
        $processes = @($webProcess)
    } else {
        Write-Host "Starting Tauri desktop shell..." -ForegroundColor Cyan
        $desktopProcess = Start-TrackedProcess `
            -Name "desktop" `
            -FilePath "bun" `
            -ArgumentList @(
                "--cwd",
                "apps/desktop",
                "tauri",
                "dev",
                "--no-watch",
                "-c",
                "src-tauri/tauri.user-test.conf.json"
            ) `
            -WorkingDirectory $repoRoot
        $processes = @($webProcess, $desktopProcess)
    }

    if ($OpenBrowser) {
        Start-Process "http://127.0.0.1:5173"
    }

    $state = [pscustomobject]@{
        startedAt = (Get-Date).ToString("o")
        mode = if ($WebOnly) { "web" } else { "desktop" }
        processes = $processes
        urls = [pscustomobject]@{
            api = "http://127.0.0.1:8080"
            web = "http://127.0.0.1:5173"
        }
    }

    $state | ConvertTo-Json -Depth 4 | Set-Content -Path $statePath

    Write-Host ""
    Write-Host "User-test stack is ready." -ForegroundColor Green
    Write-Host "API: http://127.0.0.1:8080"
    Write-Host "Web: http://127.0.0.1:5173"
    if (-not $WebOnly) {
        Write-Host "The Euripus desktop window should appear automatically."
    }
    Write-Host "Logs:"
    foreach ($processInfo in $processes) {
        Write-Host "  $($processInfo.name): $($processInfo.stdout)"
        Write-Host "  $($processInfo.name): $($processInfo.stderr)"
    }
    Write-Host "Stop everything with: bun run user-test:stop"
    exit 0
}
finally {
    Remove-Item $bootstrapStatePath -Force -ErrorAction SilentlyContinue
}
