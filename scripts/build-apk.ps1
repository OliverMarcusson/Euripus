param(
    [string]$Architecture
)

$ErrorActionPreference = "Stop"

$androidAppPath = Join-Path $PSScriptRoot "..\\apps\\android-tv-native"
$androidAppPath = [System.IO.Path]::GetFullPath($androidAppPath)
$supportedArchitectures = @("armeabi-v7a", "arm64-v8a", "x86", "x86_64")

Push-Location $androidAppPath
try {
    if (-not (Test-Path ".\\gradlew.bat")) {
        throw "Missing Gradle wrapper at $androidAppPath"
    }

    $gradleArgs = @("assembleDebug")

    if ($Architecture) {
        $selectedArchitectures = $Architecture.Split(",") |
            ForEach-Object { $_.Trim() } |
            Where-Object { $_ }

        if (-not $selectedArchitectures.Count) {
            throw "Architecture cannot be empty."
        }

        $invalidArchitectures = $selectedArchitectures |
            Where-Object { $_ -notin $supportedArchitectures }
        if ($invalidArchitectures) {
            throw "Unsupported architecture(s): $($invalidArchitectures -join ', '). Supported values: $($supportedArchitectures -join ', ')"
        }

        $gradleArgs += "-Peuripus.targetAbis=$($selectedArchitectures -join ',')"
        Write-Host "Building APK for architecture(s): $($selectedArchitectures -join ', ')"
    }
    else {
        Write-Host "Building universal APK (all supported architectures)"
    }

    & .\gradlew.bat @gradleArgs

    $apkPath = Join-Path $androidAppPath "app\\build\\outputs\\apk\\debug\\app-debug.apk"
    if (-not (Test-Path $apkPath)) {
        throw "APK not found at $apkPath"
    }

    Write-Host "APK built successfully:"
    Write-Host $apkPath
}
finally {
    Pop-Location
}
