@echo off
chcp 65001 >nul
echo ===================================
echo Camera FTP Companion - Windows Build
echo ===================================
echo.

REM Get the project directory (Windows format)
set "PROJECT_DIR=%~dp0"
cd /d "%PROJECT_DIR%"

echo Project directory: %PROJECT_DIR%
echo.

REM Check if cargo is available
where cargo >nul 2>nul
if errorlevel 1 (
    echo ERROR: cargo not found in PATH
    echo Please ensure Rust is installed on Windows
    exit /b 1
)

echo Using cargo from: 
where cargo
echo.

REM Navigate to src-tauri and build
cd src-tauri
if errorlevel 1 (
    echo ERROR: src-tauri directory not found
    exit /b 1
)

echo Building Windows executable...
echo.

REM Build the project
cargo build --release --target x86_64-pc-windows-msvc

if errorlevel 1 (
    echo.
    echo ERROR: Build failed
    exit /b 1
)

echo.
echo ===================================
echo Build completed successfully!
echo ===================================
echo.
echo Output: src-tauri\target\x86_64-pc-windows-msvc\release\camera-ftp-companion.exe

pause