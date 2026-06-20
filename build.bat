@echo off
REM =============================================
REM  pw4you - Complete Build Script
REM  Builds the application AND the installer
REM =============================================

setlocal enabledelayedexpansion
set PATH=%USERPROFILE%\.cargo\bin;%PATH%

echo.
echo  ============================================
echo    pw4you v0.1.0 - Build System
echo  ============================================
echo.

REM --- Step 0: Check prerequisites ---
echo [0/5] Checking prerequisites...

REM Check Rust
where cargo >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    echo   ERROR: Rust/Cargo not found. Install from https://rustup.rs
    exit /b 1
)
echo   Rust: OK

REM Check Inno Setup (optional - only needed for installer)
set "INNO_OK=0"
where iscc >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    set "INNO_OK=1"
    echo   Inno Setup: OK
) else (
    echo   Inno Setup: NOT FOUND (install from https://jrsoftware.org/isinfo.php for installer)
)

echo.

REM --- Step 1: Generate icon assets ---
echo [1/5] Generating icon assets...
powershell -ExecutionPolicy Bypass -File "%~dp0scripts\gen_icon.ps1" 2>nul
if %ERRORLEVEL% EQU 0 (
    echo   Icon: OK
) else (
    echo   Icon: Using existing
)
echo.

REM --- Step 2: Run tests ---
echo [2/5] Running tests...
cargo test -p pw4you-core 2>&1 | findstr /C:"test result"
if %ERRORLEVEL% NEQ 0 (
    echo   ERROR: Tests failed!
    exit /b 1
)
echo   Tests: PASSED
echo.

REM --- Step 3: Build release binary ---
echo [3/5] Building release binary (optimized for size)...
cargo build --release -p pw4you-desktop 2>&1 | findstr /C:"Finished" /C:"error"
if %ERRORLEVEL% NEQ 0 (
    echo   ERROR: Build failed!
    exit /b 1
)

REM Find the release exe
if exist "target\release\pw4you.exe" (
    for %%F in (target\release\pw4you.exe) do (
        echo   Binary: target\release\pw4you.exe
        echo   Size: %%~zF bytes ^(%%~zF KB^)
    )
) else (
    echo   ERROR: Binary not found at target\release\pw4you.exe
    exit /b 1
)
echo.

REM --- Step 4: Create distribution directory ---
echo [4/5] Preparing distribution...
if not exist "dist" mkdir dist
copy /Y "target\release\pw4you.exe" "dist\" >nul
echo   dist\pw4you.exe: ready
echo.

REM --- Step 5: Build installer (if Inno Setup available) ---
echo [5/5] Building installer...
if "%INNO_OK%"=="1" (
    iscc "installer\setup.iss" 2>&1
    if %ERRORLEVEL% EQU 0 (
        echo   Installer: dist\pw4you-setup-0.1.0.exe
    ) else (
        echo   WARNING: Installer build failed
    )
) else (
    echo   SKIPPED: Inno Setup not installed
    echo   Download from: https://jrsoftware.org/isinfo.php
    echo   Then run: iscc installer\setup.iss
)
echo.

echo  ============================================
echo    BUILD COMPLETE
echo  ============================================
echo.
echo  Output files:
echo    dist\pw4you.exe              - Standalone executable (portable)
if "%INNO_OK%"=="1" (
    echo    dist\pw4you-setup-0.1.0.exe - Graphical installer
)
echo.
echo  To install manually:
echo    powershell -File install.ps1
echo.
echo  To build the installer (requires Inno Setup):
echo    iscc installer\setup.iss
echo.

pause
