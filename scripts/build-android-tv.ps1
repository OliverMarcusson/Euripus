[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

function Assert-LastExitCode {
  param(
    [string]$Step
  )

  if ($LASTEXITCODE -ne 0) {
    throw "$Step failed with exit code $LASTEXITCODE"
  }
}

function Stop-GradleDaemons {
  $gradleJavaProcesses = Get-CimInstance Win32_Process | Where-Object {
    $_.Name -eq "java.exe" -and $_.CommandLine -match "gradle-daemon"
  }

  foreach ($process in $gradleJavaProcesses) {
    try {
      Stop-Process -Id $process.ProcessId -Force
    } catch {
      Write-Warning "Failed to stop Gradle daemon process $($process.ProcessId): $($_.Exception.Message)"
    }
  }
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$androidTwaDir = Join-Path $repoRoot "apps/android-twa"
$androidProjectDir = Join-Path $androidTwaDir "android"
$publicDir = Join-Path $repoRoot "apps/desktop/public"
$iconSourcePath = Join-Path $repoRoot "Euripus-icon.png"
$androidTvIconSourcePath = Join-Path $repoRoot "Euripus-atv-icon.png"
$icon192Path = Join-Path $publicDir "icon-192.png"
$icon512Path = Join-Path $publicDir "icon-512.png"
$configPath = Join-Path $androidTwaDir "bubblewrap.config.json"
$generatedDir = Join-Path $androidTwaDir ".generated"
$generatedIconDir = Join-Path $generatedDir "icons"
$signingDir = Join-Path $generatedDir "signing"
$keystorePath = Join-Path $signingDir "euripus-tv-release.jks"
$signingPropsPath = Join-Path $signingDir "release-signing.properties"
$manifestPath = Join-Path $generatedDir "twa-manifest.json"
$pythonLogPath = Join-Path $generatedDir "icon-server.log"
$pythonErrLogPath = Join-Path $generatedDir "icon-server.err.log"
$androidSdkRoot = Join-Path $env:USERPROFILE ".bubblewrap/android_sdk"
$sdkManager = Join-Path $androidSdkRoot "tools/bin/sdkmanager.bat"
$apksigner = Join-Path $androidSdkRoot "build-tools/35.0.0/apksigner.bat"
$licensesDir = Join-Path $androidSdkRoot "licenses"
$jdk17Path = "C:\Program Files\Microsoft\jdk-17.0.18.8-hotspot"
$keytoolPath = Join-Path $jdk17Path "bin/keytool.exe"
$port = 41731

if (!(Test-Path $configPath)) {
  throw "Missing Bubblewrap config at $configPath"
}

if (!(Test-Path $jdk17Path)) {
  throw "JDK 17 was not found at $jdk17Path"
}

if (!(Test-Path $sdkManager)) {
  throw "Android SDK manager was not found at $sdkManager"
}

if (!(Test-Path $apksigner)) {
  throw "Android APK signer was not found at $apksigner"
}

if (!(Test-Path $keytoolPath)) {
  throw "Java keytool was not found at $keytoolPath"
}

New-Item -ItemType Directory -Force -Path $generatedDir | Out-Null
New-Item -ItemType Directory -Force -Path $generatedIconDir | Out-Null
New-Item -ItemType Directory -Force -Path $signingDir | Out-Null

python -c "import PIL" 2>$null
if ($LASTEXITCODE -ne 0) {
  python -m pip install Pillow | Out-Host
}

if (Test-Path $iconSourcePath) {
  @'
from PIL import Image
from pathlib import Path

source = Path(r'__SOURCE__')
public_dir = Path(r'__PUBLIC_DIR__')

with Image.open(source) as img:
    img = img.convert("RGBA")
    for size in (512, 192):
        target = public_dir / f"icon-{size}.png"
        img.resize((size, size), Image.Resampling.LANCZOS).save(target)
'@.Replace('__SOURCE__', $iconSourcePath.Replace('\', '\\')).Replace('__PUBLIC_DIR__', $publicDir.Replace('\', '\\')) | python - | Out-Host
} elseif (!(Test-Path $icon192Path) -or !(Test-Path $icon512Path)) {
  @'
from PIL import Image, ImageDraw
from pathlib import Path

root = Path(r'__PUBLIC_DIR__')
BG = '#140B2B'
PURPLE = '#8E37DA'
SCREEN = '#12081E'
ORANGE = '#FF7451'
WHITE = '#F6F0FF'

for size in (512, 192):
    scale = size / 512
    img = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    def s(v):
        return round(v * scale)

    draw.rounded_rectangle((0, 0, size - 1, size - 1), radius=s(120), fill=BG)
    draw.rounded_rectangle((s(78), s(112), s(434), s(348)), radius=s(44), fill=PURPLE)
    draw.rounded_rectangle((s(110), s(144), s(402), s(316)), radius=s(28), fill=SCREEN)

    for cx in (182, 255, 328):
        r = s(22)
        x = s(cx)
        y = s(230)
        draw.ellipse((x-r, y-r, x+r, y+r), fill=ORANGE)

    draw.line((s(170), s(392), s(342), s(392)), fill=WHITE, width=max(1, s(28)))
    draw.line((s(222), s(86), s(160), s(38)), fill=WHITE, width=max(1, s(20)))
    draw.line((s(290), s(86), s(352), s(38)), fill=WHITE, width=max(1, s(20)))

    img.save(root / f'icon-{size}.png')
'@.Replace('__PUBLIC_DIR__', $publicDir.Replace('\', '\\')) | python - | Out-Host
}

if (Test-Path $androidTvIconSourcePath) {
  @'
from PIL import Image
from pathlib import Path

source = Path(r'__SOURCE__')
target = Path(r'__TARGET__')

with Image.open(source) as img:
    img.convert("RGBA").resize((512, 512), Image.Resampling.LANCZOS).save(target)
'@.Replace('__SOURCE__', $androidTvIconSourcePath.Replace('\', '\\')).Replace('__TARGET__', (Join-Path $generatedIconDir "android-tv-icon-512.png").Replace('\', '\\')) | python - | Out-Host
} else {
  Copy-Item -Path $icon512Path -Destination (Join-Path $generatedIconDir "android-tv-icon-512.png") -Force
}

if (!(Test-Path $signingPropsPath)) {
  $password = [Guid]::NewGuid().ToString("N") + [Guid]::NewGuid().ToString("N")
  @"
storeFile=$keystorePath
storePassword=$password
keyAlias=euripus-tv
keyPassword=$password
"@ | Set-Content -Path $signingPropsPath -NoNewline
}

$signingProps = @{}
Get-Content $signingPropsPath | ForEach-Object {
  if ($_ -match '^\s*([^=]+?)\s*=\s*(.*)\s*$') {
    $signingProps[$matches[1]] = $matches[2]
  }
}

if (!(Test-Path $signingProps["storeFile"])) {
  & $keytoolPath -genkeypair `
    -keystore $signingProps["storeFile"] `
    -storepass $signingProps["storePassword"] `
    -keypass $signingProps["keyPassword"] `
    -alias $signingProps["keyAlias"] `
    -keyalg RSA `
    -keysize 2048 `
    -validity 3650 `
    -dname "CN=Euripus Android TV, OU=Local Build, O=Euripus, L=Stockholm, S=Stockholm, C=SE" | Out-Host
}

$config = Get-Content $configPath -Raw | ConvertFrom-Json

$twaManifest = [ordered]@{
  packageId = $config.packageId
  host = $config.host
  name = $config.name
  launcherName = $config.launcherName
  display = $config.display
  themeColor = $config.themeColor
  themeColorDark = if ($config.PSObject.Properties.Name -contains "themeColorDark") { $config.themeColorDark } else { "#000000" }
  navigationColor = if ($config.PSObject.Properties.Name -contains "navigationColor") { $config.navigationColor } else { $config.backgroundColor }
  navigationColorDark = if ($config.PSObject.Properties.Name -contains "navigationColorDark") { $config.navigationColorDark } else { $config.backgroundColor }
  navigationDividerColor = "#00000000"
  navigationDividerColorDark = "#00000000"
  backgroundColor = $config.backgroundColor
  enableNotifications = [bool]$config.enableNotifications
  enableSiteSettingsShortcut = $false
  startUrl = $config.startUrl
  iconUrl = "http://127.0.0.1:$port/android-tv-icon-512.png"
  maskableIconUrl = "http://127.0.0.1:$port/android-tv-icon-512.png"
  appVersion = $config.appVersionName
  appVersionCode = [int]$config.appVersionCode
  splashScreenFadeOutDuration = 300
  fallbackType = if ($config.PSObject.Properties.Name -contains "fallbackType") { $config.fallbackType } else { "customtabs" }
  isChromeOSOnly = [bool]$config.isChromeOSOnly
  orientation = if ($config.PSObject.Properties.Name -contains "orientation") { $config.orientation } else { "landscape" }
  generatorApp = "bubblewrap-cli"
  signingKey = @{
    path = "./android.keystore"
    alias = "android"
  }
}

$twaManifest | ConvertTo-Json -Depth 8 | Set-Content -Path $manifestPath -NoNewline

$pythonProcess = $null
try {
  $pythonProcess = Start-Process `
    -FilePath "python" `
    -ArgumentList @("-m", "http.server", "$port", "--bind", "127.0.0.1") `
    -WorkingDirectory $generatedIconDir `
    -RedirectStandardOutput $pythonLogPath `
    -RedirectStandardError $pythonErrLogPath `
    -PassThru

  Start-Sleep -Seconds 2

  try {
    Invoke-WebRequest -Uri "http://127.0.0.1:$port/android-tv-icon-512.png" -UseBasicParsing | Out-Null
  } catch {
    throw "Temporary icon server did not start successfully."
  }

  $env:JAVA_HOME = $jdk17Path
  $env:PATH = "$env:JAVA_HOME\bin;$env:PATH"
  $env:ANDROID_HOME = $androidSdkRoot

  New-Item -ItemType Directory -Force -Path $licensesDir | Out-Null
  Set-Content -Path (Join-Path $licensesDir "android-sdk-license") -Value @(
    "24333f8a63b6825ea9c5514f83c2829b004d1fee",
    "d56f5187479451eabf01fb78af6dfcb131a6481e"
  )
  Set-Content -Path (Join-Path $licensesDir "android-sdk-preview-license") -Value "84831b9409646a918e30573bab4c9c91346d8abd"

  & $sdkManager --sdk_root=$androidSdkRoot --install "platform-tools" "platforms;android-36" "build-tools;35.0.0" | Out-Host
  Assert-LastExitCode "Android SDK install"

  if (Test-Path (Join-Path $androidProjectDir "gradlew.bat")) {
    Push-Location $androidProjectDir
    try {
      & ".\gradlew.bat" --stop | Out-Host
    } finally {
      Pop-Location
    }
  }

  Stop-GradleDaemons
  Start-Sleep -Seconds 2

  foreach ($path in @(
    (Join-Path $androidProjectDir "app/build"),
    (Join-Path $androidProjectDir "build"),
    (Join-Path $androidProjectDir ".gradle")
  )) {
    if (Test-Path $path) {
      Remove-Item -LiteralPath $path -Recurse -Force
    }
  }

  npx --yes @bubblewrap/cli update --skipVersionUpgrade --directory $androidProjectDir --manifest $manifestPath
  Assert-LastExitCode "Bubblewrap update"

  $androidManifestPath = Join-Path $androidProjectDir "app/src/main/AndroidManifest.xml"
  $androidManifest = Get-Content $androidManifestPath -Raw

  if ($androidManifest -notmatch 'android\.hardware\.touchscreen') {
    $androidManifest = $androidManifest -replace '<application', "<uses-feature android:name=`"android.hardware.touchscreen`" android:required=`"false`" />`r`n`r`n    <uses-feature android:name=`"android.software.leanback`" android:required=`"false`" />`r`n`r`n    <application"
  }

  if ($androidManifest -notmatch 'LEANBACK_LAUNCHER') {
    $androidManifest = $androidManifest -replace '<category android:name="android.intent.category.LAUNCHER" />', "<category android:name=`"android.intent.category.LAUNCHER`" />`r`n                <category android:name=`"android.intent.category.LEANBACK_LAUNCHER`" />"
  }

  Set-Content -Path $androidManifestPath -Value $androidManifest -NoNewline

  Push-Location $androidProjectDir
  try {
    & ".\gradlew.bat" assembleDebug
    Assert-LastExitCode "Gradle assembleDebug"
    & ".\gradlew.bat" assembleRelease
    Assert-LastExitCode "Gradle assembleRelease"
  } finally {
    Pop-Location
  }

  $unsignedReleaseApk = Join-Path $androidProjectDir "app/build/outputs/apk/release/app-release-unsigned.apk"
  $signedReleaseApk = Join-Path $androidProjectDir "app/build/outputs/apk/release/app-release-signed.apk"

  if (!(Test-Path $unsignedReleaseApk)) {
    throw "Expected unsigned release APK at $unsignedReleaseApk"
  }

  if (Test-Path $signedReleaseApk) {
    Remove-Item -LiteralPath $signedReleaseApk -Force
  }

  & $apksigner sign `
    --ks $signingProps["storeFile"] `
    --ks-key-alias $signingProps["keyAlias"] `
    --ks-pass "pass:$($signingProps["storePassword"])" `
    --key-pass "pass:$($signingProps["keyPassword"])" `
    --out $signedReleaseApk `
    $unsignedReleaseApk | Out-Host
  Assert-LastExitCode "APK signing"

  & $apksigner verify --verbose $signedReleaseApk | Out-Host
  Assert-LastExitCode "APK verification"
} finally {
  if ($pythonProcess -and !$pythonProcess.HasExited) {
    Stop-Process -Id $pythonProcess.Id -Force
  }
}
