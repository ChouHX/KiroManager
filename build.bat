@echo off
REM ========================================================================
REM  Kiro Manager (Tauri 2) - Windows release 构建
REM  需要: VS 2022 (MSVC) + Node.js + Rust
REM ========================================================================
setlocal EnableDelayedExpansion
pushd "%~dp0"

REM 代理 (如不需要可删掉)
set HTTP_PROXY=http://127.0.0.1:7897
set HTTPS_PROXY=http://127.0.0.1:7897

REM 设置 MSVC 环境
for /f "tokens=*" %%i in ('"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe" -latest -property installationPath') do set VS_ROOT=%%i
if not defined VS_ROOT (
    echo [x] 未找到 Visual Studio
    goto :fail
)
call "%VS_ROOT%\VC\Auxiliary\Build\vcvarsall.bat" x64 >nul
if errorlevel 1 (
    echo [x] vcvarsall.bat 失败
    goto :fail
)

REM 强制 cc-rs 使用 MSVC 工具 (避免 PATH 中 MinGW 干扰)
set CC=cl.exe
set CXX=cl.exe
set AR=lib.exe

echo [OK] MSVC 环境已加载

echo.
echo [1/3] npm install ...
call npm install
if errorlevel 1 goto :fail

echo.
echo [2/3] 构建 CSS (Tailwind) ...
call npx @tailwindcss/cli -i src/input.css -o src/style.css --minify
if errorlevel 1 goto :fail

echo.
echo [3/3] cargo build --release ...
cd /d "%~dp0src-tauri"
cargo build --release
if errorlevel 1 goto :fail
cd /d "%~dp0"

echo.
echo [4/4] 复制产物 ...
if not exist "dist" mkdir "dist"
copy /Y "src-tauri\target\release\kiro-manager.exe" "dist\KiroManager.exe" >nul

for %%F in ("dist\KiroManager.exe") do set "SZ=%%~zF"
set /a MB=%SZ% / 1048576

echo.
echo ========================================================================
echo  构建完成!
echo  产物: %CD%\dist\KiroManager.exe  (~%MB% MB)
echo  注意: 运行需要系统已安装 WebView2 (Win10/11 自带)
echo ========================================================================
popd
endlocal
exit /b 0

:fail
echo [X] 构建失败
popd
endlocal
exit /b 1
