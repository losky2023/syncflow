$ErrorActionPreference = "Stop"

$clientRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$pidFile = Join-Path $clientRoot ".syncflow-tauri-dev.pid"

& (Join-Path $PSScriptRoot "stop-tauri-dev.ps1")

Set-Location $clientRoot
$process = Start-Process powershell `
    -WindowStyle Hidden `
    -PassThru `
    -ArgumentList @(
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        "cd '$clientRoot'; npx tauri dev"
    )

Set-Content -LiteralPath $pidFile -Value $process.Id -Encoding ascii
Write-Host "Started SyncFlow Tauri dev host pid=$($process.Id)"
