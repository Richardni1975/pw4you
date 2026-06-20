@echo off
REM pw4you Setup Launcher
REM Double-click this file to start the graphical installer.

set "SCRIPT_DIR=%~dp0"
powershell -ExecutionPolicy Bypass -WindowStyle Normal -File "%SCRIPT_DIR%gui-installer.ps1"
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Installation failed or was cancelled.
    pause
)
