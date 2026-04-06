$ErrorActionPreference = "Stop"

$androidAppPath = Join-Path $PSScriptRoot "..\\apps\\android-tv-native"
$androidAppPath = [System.IO.Path]::GetFullPath($androidAppPath)

Push-Location $androidAppPath
try {
    if (-not (Test-Path ".\\gradlew.bat")) {
        throw "Missing Gradle wrapper at $androidAppPath"
    }

    & .\gradlew.bat assembleDebug

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
