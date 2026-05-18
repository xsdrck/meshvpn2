@echo off
chcp 65001 > nul
echo ========================================
echo MeshVPN - Windows Build Script
echo ========================================
echo.

echo [1/3] Checking environment...
rustc --version
if errorlevel 1 (
    echo ERROR: Rust not detected! Please install first.
    echo Visit https://rustup.rs/ to download and install.
    pause
    exit /b 1
)

cargo --version
echo.

echo [2/3] Starting build...
echo This may take a few minutes, please wait...
echo.
cargo build --release
if errorlevel 1 (
    echo.
    echo ERROR: Build failed!
    pause
    exit /b 1
)

echo.
echo [3/3] Build successful!
echo.
echo ========================================
echo Executable location:
echo target\release\meshvpn.exe
echo.
echo ========================================
echo Quick test:
echo target\release\meshvpn.exe --help
echo.

pause
