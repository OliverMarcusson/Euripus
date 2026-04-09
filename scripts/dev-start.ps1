param(
    [switch]$NoBuild,
    [switch]$OpenBrowser
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$runtimeDir = Join-Path $repoRoot ".runtime"
$logsDir = Join-Path $runtimeDir "logs"
$statePath = Join-Path $runtimeDir "dev-stack.json"
$bootstrapStatePath = Join-Path $runtimeDir "dev-bootstrap.json"

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

function Get-LocalIpv4Addresses {
    $addresses = Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
        Where-Object {
            $_.IPAddress -notlike "127.*" -and
            $_.IPAddress -notlike "169.254.*" -and
            $_.PrefixOrigin -ne "WellKnown"
        } |
        Select-Object -ExpandProperty IPAddress -Unique

    return @($addresses)
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

function Get-ComposeServiceContainerId {
    param([string]$ServiceName)

    $containerId = (& docker compose ps -q $ServiceName 2>$null | Select-Object -First 1)
    if (-not $containerId) {
        return $null
    }

    return $containerId.Trim()
}

function Get-ContainerStatus {
    param([string]$ContainerId)

    if (-not $ContainerId) {
        return $null
    }

    try {
        return (& docker inspect -f "{{.State.Status}}" $ContainerId 2>$null | Select-Object -First 1).Trim()
    } catch {
        return $null
    }
}

function Get-ServerLogs {
    return (& docker compose logs --tail 200 server 2>&1 | Out-String)
}

function Wait-ForApiHealth {
    param([int]$TimeoutSeconds = 180)

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $resetAttempted = $false

    while ((Get-Date) -lt $deadline) {
        try {
            Invoke-WebRequest -Uri "http://127.0.0.1:8080/health" -UseBasicParsing -TimeoutSec 5 | Out-Null
            return
        } catch {
            $serverContainerId = Get-ComposeServiceContainerId -ServiceName "server"
            $serverStatus = Get-ContainerStatus -ContainerId $serverContainerId

            if ($serverStatus -eq "exited") {
                $logs = Get-ServerLogs
                if ((-not $resetAttempted) -and $logs -match "migration .* previously applied but has been modified") {
                    $resetAttempted = $true
                    Write-Host "Detected local migration checksum drift in the dev database. Recreating the local database volume..." -ForegroundColor Yellow
                    & docker compose down -v | Out-Host
                    & docker compose up --build -d postgres server | Out-Host
                    Start-Sleep -Seconds 2
                    continue
                }

                throw "API container exited before becoming healthy.`n`nServer logs:`n$logs"
            }
        }

        Start-Sleep -Milliseconds 750
    }

    $logs = Get-ServerLogs
    throw "API did not become ready at http://127.0.0.1:8080/health within $TimeoutSeconds seconds.`n`nServer logs:`n$logs"
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
        throw "Missing apps/server/.env. Copy apps/server/.env.example before starting the dev stack."
    }

    if (Test-Path $statePath) {
        $existingState = Get-Content $statePath | ConvertFrom-Json
        $clientProcess = $existingState.processes | Where-Object { $_.name -eq "client" -or $_.name -eq "web" } | Select-Object -First 1
        if (Test-TrackedProcessRunning $clientProcess) {
            Write-Host "Dev stack is already running." -ForegroundColor Yellow
            Write-Host "Stop it first with: bun run dev:stop"
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
    Wait-ForApiHealth -TimeoutSeconds 180

    Write-Host "Starting web client..." -ForegroundColor Cyan
    $clientProcess = Start-TrackedProcess `
        -Name "client" `
        -FilePath "bun" `
        -ArgumentList @("--cwd", "apps/client", "dev", "--host", "0.0.0.0") `
        -WorkingDirectory $repoRoot

    Write-Host "Waiting for frontend dev server..." -ForegroundColor Cyan
    Wait-ForHttpEndpoint -Name "Frontend" -Url "http://127.0.0.1:5173" -TimeoutSeconds 180

    $processes = @($clientProcess)

    if ($OpenBrowser) {
        Start-Process "http://127.0.0.1:5173"
    }

    $state = [pscustomobject]@{
        startedAt = (Get-Date).ToString("o")
        mode = "web"
        processes = $processes
        urls = [pscustomobject]@{
            api = "http://127.0.0.1:8080"
            web = "http://127.0.0.1:5173"
        }
    }

    $state | ConvertTo-Json -Depth 4 | Set-Content -Path $statePath

    Write-Host ""
    Write-Host "Dev stack is ready." -ForegroundColor Green
    Write-Host "API: http://127.0.0.1:8080"
    Write-Host "Web: http://127.0.0.1:5173"
    $lanAddresses = Get-LocalIpv4Addresses
    if ($lanAddresses.Count -gt 0) {
        Write-Host "LAN URLs:" -ForegroundColor Cyan
        foreach ($address in $lanAddresses) {
            Write-Host "  Web: http://$address:5173"
        }
    }
    Write-Host "Logs:"
    foreach ($processInfo in $processes) {
        Write-Host "  $($processInfo.name): $($processInfo.stdout)"
        Write-Host "  $($processInfo.name): $($processInfo.stderr)"
    }
    Write-Host "Stop everything with: bun run dev:stop"
    exit 0
}
finally {
    Remove-Item $bootstrapStatePath -Force -ErrorAction SilentlyContinue
}
