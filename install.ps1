# pw4you Installer Script
# Run as Administrator for full functionality
# Usage: powershell -ExecutionPolicy Bypass -File install.ps1

param(
    [switch]$Uninstall,
    [string]$InstallPath = "$env:LOCALAPPDATA\pw4you"
)

$ErrorActionPreference = "Stop"
$AppName = "pw4you"
$AppExe = "pw4you.exe"
$Version = "0.1.0"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  pw4you v$Version - 文件夹保险箱" -ForegroundColor Cyan
Write-Host "  Installer / Uninstaller" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

if ($Uninstall) {
    Write-Host "[UNINSTALL] Removing pw4you..." -ForegroundColor Yellow

    # Remove install directory
    if (Test-Path $InstallPath) {
        Remove-Item -Recurse -Force $InstallPath
        Write-Host "  Removed: $InstallPath"
    }

    # Remove Start Menu shortcut
    $startMenu = [Environment]::GetFolderPath("StartMenu") + "\Programs\pw4you"
    if (Test-Path $startMenu) {
        Remove-Item -Recurse -Force $startMenu
        Write-Host "  Removed: Start Menu shortcut"
    }

    # Remove Desktop shortcut
    $desktop = [Environment]::GetFolderPath("Desktop") + "\pw4you.lnk"
    if (Test-Path $desktop) {
        Remove-Item -Force $desktop
        Write-Host "  Removed: Desktop shortcut"
    }

    # Remove registry entries
    $regPath = "HKCU:\Software\pw4you"
    if (Test-Path $regPath) {
        Remove-Item -Recurse -Force $regPath
        Write-Host "  Removed: Registry settings"
    }

    Write-Host "[DONE] pw4you has been uninstalled." -ForegroundColor Green
    exit 0
}

Write-Host "[INSTALL] Installing pw4you v$Version..." -ForegroundColor Cyan
Write-Host "  Install path: $InstallPath"
Write-Host ""

# Step 1: Create install directory
Write-Host "[1/5] Creating directories..."
New-Item -ItemType Directory -Force -Path $InstallPath | Out-Null
New-Item -ItemType Directory -Force -Path "$InstallPath\data" | Out-Null
Write-Host "  OK"

# Step 2: Copy executable
Write-Host "[2/5] Copying executable..."
$sourceExe = Join-Path $PSScriptRoot "target\release\$AppExe"
if (-not (Test-Path $sourceExe)) {
    $sourceExe = Join-Path $PSScriptRoot "..\..\..\..\..\Users\$env:USERNAME\pw4you_target\release\$AppExe"
}
if (-not (Test-Path $sourceExe)) {
    Write-Error "Cannot find pw4you.exe. Build it first with: cargo build --release -p pw4you-desktop"
    exit 1
}
Copy-Item $sourceExe $InstallPath
Write-Host "  OK ($([math]::Round((Get-Item $sourceExe).Length/1MB, 2)) MB)"

# Step 3: Register shell extension
Write-Host "[3/5] Registering .pw4 file association..."
$installExe = Join-Path $InstallPath $AppExe
& $installExe 2>$null  # Run once to register shell extension
Write-Host "  OK"

# Step 4: Create shortcuts
Write-Host "[4/5] Creating shortcuts..."

# Start Menu
$startMenuDir = [Environment]::GetFolderPath("StartMenu") + "\Programs\pw4you"
New-Item -ItemType Directory -Force -Path $startMenuDir | Out-Null

$WshShell = New-Object -ComObject WScript.Shell

# Start Menu shortcut
$shortcut = $WshShell.CreateShortcut("$startMenuDir\pw4you.lnk")
$shortcut.TargetPath = $installExe
$shortcut.WorkingDirectory = $InstallPath
$shortcut.Description = "pw4you - 文件夹保险箱"
$shortcut.Save()

# Desktop shortcut
$desktopShortcut = $WshShell.CreateShortcut("$([Environment]::GetFolderPath('Desktop'))\pw4you.lnk")
$desktopShortcut.TargetPath = $installExe
$desktopShortcut.WorkingDirectory = $InstallPath
$desktopShortcut.Description = "pw4you - 安全的文件夹加密工具"
$desktopShortcut.Save()

Write-Host "  OK"

# Step 5: Register autostart (optional)
Write-Host "[5/5] Registering autostart..."
$startup = [Environment]::GetFolderPath("Startup")
$startupShortcut = $WshShell.CreateShortcut("$startup\pw4you.lnk")
$startupShortcut.TargetPath = $installExe
$startupShortcut.WorkingDirectory = $InstallPath
$startupShortcut.Description = "pw4you - 开机自动启动"
$startupShortcut.WindowStyle = 7  # Minimized
$startupShortcut.Save()
Write-Host "  OK (remove from Startup folder to disable)"

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "  pw4you installed successfully!" -ForegroundColor Green
Write-Host "  Location: $InstallExe" -ForegroundColor Green
Write-Host "  Uninstall: powershell -File install.ps1 -Uninstall" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
