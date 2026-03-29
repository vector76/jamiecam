@echo off
setlocal enabledelayedexpansion

:: ── Parse arguments ──────────────────────────────────────────────────
set "MODE=debug"
set "SKIP_ENV_CHECK="

:parse_args
if "%~1"=="" goto args_done
if /i "%~1"=="debug"     ( set "MODE=debug"     & shift & goto parse_args )
if /i "%~1"=="release"   ( set "MODE=release"   & shift & goto parse_args )
if /i "%~1"=="installer" ( set "MODE=installer" & shift & goto parse_args )
if /i "%~1"=="--skip-env-check" ( set "SKIP_ENV_CHECK=1" & shift & goto parse_args )
if /i "%~1"=="--help" goto usage
if /i "%~1"=="-h"     goto usage
echo Unknown argument: %~1
goto usage
:args_done

:: ── Banner ───────────────────────────────────────────────────────────
echo ============================================
if "%MODE%"=="debug"     echo  JamieCam Build [debug]
if "%MODE%"=="release"   echo  JamieCam Build [release]
if "%MODE%"=="installer" echo  JamieCam Build [release + installer]
echo ============================================
echo.

:: ── Check environment ────────────────────────────────────────────────
if defined SKIP_ENV_CHECK (
    echo Skipping environment check (--skip-env-check)
    echo.
) else (
    echo [1/3] Checking environment...
    powershell -ExecutionPolicy Bypass -File "%~dp0scripts\check-env.ps1"
    if !ERRORLEVEL! neq 0 (
        echo.
        echo Build aborted: environment check failed.
        echo Tip: use --skip-env-check to bypass this step.
        exit /b 1
    )
    echo.
)

:: ── Install frontend dependencies ────────────────────────────────────
echo [2/3] Installing frontend dependencies...
call pnpm install --frozen-lockfile
if %ERRORLEVEL% neq 0 (
    echo.
    echo Build aborted: pnpm install failed.
    exit /b 1
)
echo.

:: ── Build ────────────────────────────────────────────────────────────
if "%MODE%"=="debug" (
    echo [3/3] Building Tauri application [debug, no bundling]...
    call pnpm tauri build --debug --no-bundle
    if !ERRORLEVEL! neq 0 (
        echo.
        echo Build FAILED.
        exit /b 1
    )
    echo.
    echo ============================================
    echo  Build succeeded! [debug]
    echo ============================================
    echo.
    echo Executable:
    echo   src-tauri\target\debug\JamieCam.exe
)

if "%MODE%"=="release" (
    echo [3/3] Building Tauri application [release, no bundling]...
    call pnpm tauri build --no-bundle
    if !ERRORLEVEL! neq 0 (
        echo.
        echo Build FAILED.
        exit /b 1
    )
    echo.
    :: ── UPX compression ────────────────────────────────────────────────
    where upx >nul 2>nul
    if !ERRORLEVEL! equ 0 (
        echo Compressing executable with UPX...
        upx --best --lzma "src-tauri\target\release\JamieCam.exe"
        if !ERRORLEVEL! neq 0 (
            echo WARNING: UPX compression failed ^(non-fatal^).
        )
    ) else (
        echo NOTE: UPX not found in PATH; skipping compression.
    )
    echo.
    echo ============================================
    echo  Build succeeded! [release]
    echo ============================================
    echo.
    echo Executable:
    echo   src-tauri\target\release\JamieCam.exe
)

if "%MODE%"=="installer" (
    echo [3/3] Building Tauri application [release + installers]...
    call pnpm tauri build
    if !ERRORLEVEL! neq 0 (
        echo.
        echo Build FAILED.
        exit /b 1
    )
    echo.
    :: ── UPX compression (standalone exe only; installers use own compression) ──
    where upx >nul 2>nul
    if !ERRORLEVEL! equ 0 (
        echo Compressing standalone executable with UPX...
        upx --best --lzma "src-tauri\target\release\JamieCam.exe"
        if !ERRORLEVEL! neq 0 (
            echo WARNING: UPX compression failed ^(non-fatal^).
        )
    ) else (
        echo NOTE: UPX not found in PATH; skipping compression.
    )
    echo.
    echo ============================================
    echo  Build succeeded! [release + installers]
    echo ============================================
    echo.
    echo Executable:
    echo   src-tauri\target\release\JamieCam.exe
    echo.
    echo Installers:
    for %%f in ("src-tauri\target\release\bundle\nsis\*.exe") do echo   %%f
    for %%f in ("src-tauri\target\release\bundle\msi\*.msi") do echo   %%f
)

echo.
exit /b 0

:: ── Usage ────────────────────────────────────────────────────────────
:usage
echo.
echo Usage: build.bat [mode] [options]
echo.
echo Modes:
echo   debug       Fast incremental build (default)
echo   release     Optimized release exe, no installer
echo   installer   Full release build with MSI/NSIS installers
echo.
echo Options:
echo   --skip-env-check   Skip the environment check step
echo   -h, --help         Show this help
echo.
echo Examples:
echo   build                 Debug build (fastest)
echo   build release         Release exe only
echo   build installer       Release + MSI/NSIS
echo.
exit /b 0
