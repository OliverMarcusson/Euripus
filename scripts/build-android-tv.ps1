[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$androidTwaDir = Join-Path $repoRoot "apps/android-twa"
$androidProjectDir = Join-Path $androidTwaDir "android"
$publicDir = Join-Path $repoRoot "apps/desktop/public"
$icon192Path = Join-Path $publicDir "icon-192.png"
$icon512Path = Join-Path $publicDir "icon-512.png"
$configPath = Join-Path $androidTwaDir "bubblewrap.config.json"
$generatedDir = Join-Path $androidTwaDir ".generated"
$manifestPath = Join-Path $generatedDir "twa-manifest.json"
$pythonLogPath = Join-Path $generatedDir "icon-server.log"
$pythonErrLogPath = Join-Path $generatedDir "icon-server.err.log"
$androidSdkRoot = Join-Path $env:USERPROFILE ".bubblewrap/android_sdk"
$sdkManager = Join-Path $androidSdkRoot "tools/bin/sdkmanager.bat"
$licensesDir = Join-Path $androidSdkRoot "licenses"
$jdk17Path = "C:\Program Files\Microsoft\jdk-17.0.18.8-hotspot"
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

New-Item -ItemType Directory -Force -Path $generatedDir | Out-Null

if (!(Test-Path $icon192Path) -or !(Test-Path $icon512Path)) {
  python -c "import PIL" 2>$null
  if ($LASTEXITCODE -ne 0) {
    python -m pip install Pillow | Out-Host
  }

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
  iconUrl = "http://127.0.0.1:$port/icon-512.png"
  maskableIconUrl = "http://127.0.0.1:$port/icon-512.png"
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
    -WorkingDirectory $publicDir `
    -RedirectStandardOutput $pythonLogPath `
    -RedirectStandardError $pythonErrLogPath `
    -PassThru

  Start-Sleep -Seconds 2

  try {
    Invoke-WebRequest -Uri "http://127.0.0.1:$port/icon-512.png" -UseBasicParsing | Out-Null
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

  npx --yes @bubblewrap/cli update --skipVersionUpgrade --directory $androidProjectDir --manifest $manifestPath

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
  } finally {
    Pop-Location
  }
} finally {
  if ($pythonProcess -and !$pythonProcess.HasExited) {
    Stop-Process -Id $pythonProcess.Id -Force
  }
}
