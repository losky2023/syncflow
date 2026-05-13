$ErrorActionPreference = "Continue"

$clientRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$pidFile = Join-Path $clientRoot ".syncflow-tauri-dev.pid"

if (Test-Path -LiteralPath $pidFile) {
    $pidText = (Get-Content -LiteralPath $pidFile -Raw).Trim()
    if ($pidText -match "^\d+$") {
        $process = Get-Process -Id ([int]$pidText) -ErrorAction SilentlyContinue
        if ($process) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            Write-Host "Stopped SyncFlow Tauri dev host pid=$($process.Id)"
        }
    }
    Remove-Item -LiteralPath $pidFile -Force -ErrorAction SilentlyContinue
}

Get-Process syncflow -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue
