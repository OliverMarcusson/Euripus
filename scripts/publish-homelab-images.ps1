param(
    [string]$Platform = "linux/amd64",
    [string]$MovingTag = $(if ($env:EURIPUS_IMAGE_TAG) { $env:EURIPUS_IMAGE_TAG } else { "homelab-latest" }),
    [string]$ServerImage = $(if ($env:EURIPUS_SERVER_IMAGE) { $env:EURIPUS_SERVER_IMAGE } else { "ghcr.io/olivermarcusson/euripus-server" }),
    [string]$WebImage = $(if ($env:EURIPUS_WEB_IMAGE) { $env:EURIPUS_WEB_IMAGE } else { "ghcr.io/olivermarcusson/euripus-web" })
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot

function Assert-CommandAvailable {
    param([string]$CommandName)

    if (-not (Get-Command $CommandName -ErrorAction SilentlyContinue)) {
        throw "Required command '$CommandName' was not found on PATH."
    }
}

function Get-GitSha {
    $sha = (& git -C $repoRoot rev-parse HEAD | Select-Object -First 1)
    if (-not $sha) {
        throw "Unable to resolve the current git SHA."
    }

    return $sha.Trim()
}

function Invoke-GhcrLoginIfConfigured {
    if (-not $env:GHCR_USERNAME -or -not $env:GHCR_TOKEN) {
        Write-Host "GHCR_USERNAME and GHCR_TOKEN are not both set. Assuming you already ran 'docker login ghcr.io'." -ForegroundColor Yellow
        return
    }

    $env:GHCR_TOKEN | docker login ghcr.io --username $env:GHCR_USERNAME --password-stdin | Out-Host
}

function Publish-Image {
    param(
        [string]$ImageName,
        [string]$DockerfilePath,
        [string]$ShaTag,
        [string]$MovingTagValue
    )

    $tagArgs = @(
        "--tag", "${ImageName}:${ShaTag}",
        "--tag", "${ImageName}:${MovingTagValue}"
    )

    $args = @(
        "buildx",
        "build",
        "--platform",
        $Platform,
        "--file",
        $DockerfilePath
    ) + $tagArgs + @(
        "--push",
        $repoRoot
    )

    Write-Host "Publishing ${ImageName}:${ShaTag} and ${ImageName}:${MovingTagValue}..." -ForegroundColor Cyan
    & docker @args | Out-Host
}

Assert-CommandAvailable "docker"
Assert-CommandAvailable "git"

$shaTag = Get-GitSha

Invoke-GhcrLoginIfConfigured

Publish-Image -ImageName $ServerImage -DockerfilePath "apps/server/Dockerfile" -ShaTag $shaTag -MovingTagValue $MovingTag
Publish-Image -ImageName $WebImage -DockerfilePath "apps/web/Dockerfile" -ShaTag $shaTag -MovingTagValue $MovingTag

Write-Host ""
Write-Host "Published homelab images." -ForegroundColor Green
Write-Host "Server: ${ServerImage}:${shaTag}"
Write-Host "Server: ${ServerImage}:${MovingTag}"
Write-Host "Web: ${WebImage}:${shaTag}"
Write-Host "Web: ${WebImage}:${MovingTag}"
