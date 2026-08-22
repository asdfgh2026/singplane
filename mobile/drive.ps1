# Drive the Compose skeleton on a running emulator (AVD: singpanel_api35).
param(
    [switch]$StartEmu
)

$ErrorActionPreference = "Stop"
$Sdk = $env:ANDROID_HOME
if (-not $Sdk) { $Sdk = Join-Path $env:USERPROFILE "scoop\apps\android-clt\current" }
$env:ANDROID_HOME = $Sdk
$env:ANDROID_SDK_ROOT = $Sdk
$env:PATH = "$Sdk\platform-tools;$Sdk\emulator;$env:PATH"

$Apk = Join-Path $PSScriptRoot "app\build\outputs\apk\debug\app-debug.apk"
$Pkg = "app.singplane.debug"
$Act = "$Pkg/app.singplane.MainActivity"

if ($StartEmu) {
    $env:ANDROID_EMU_ENABLE_CRASH_REPORTING = "0"
    Start-Process -FilePath (Join-Path $Sdk "emulator\emulator.exe") -ArgumentList @(
        "-avd", "singpanel_api35",
        "-gpu", "swiftshader_indirect",
        "-no-snapshot",
        "-no-boot-anim",
        "-no-audio"
    )
}

adb wait-for-device
$deadline = (Get-Date).AddMinutes(4)
do {
    $boot = (adb shell getprop sys.boot_completed 2>$null | Out-String).Trim()
    if ($boot -eq "1") { break }
    Start-Sleep -Seconds 3
} while ((Get-Date) -lt $deadline)

if (-not (Test-Path $Apk)) { throw "APK missing. Run: .\gradlew :app:assembleDebug" }
adb install -r $Apk
adb shell am start -n $Act
Write-Host "Launched $Act"
Write-Host "Manual: tap 电源 / 底栏. Screenshots from last run: mobile\drive\"
