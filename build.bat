@echo off
setlocal EnableDelayedExpansion
title QuickSnip Recorder - Rust Multi-Threaded Builder
color 0B

cd /d "%~dp0"

set "JOBS=%NUMBER_OF_PROCESSORS%"
if "%JOBS%"=="" set "JOBS=4"
set "CARGO_BUILD_JOBS=%JOBS%"
set "CARGO_INCREMENTAL=0"
set "CARGO_HTTP_MULTIPLEXING=true"
set "CARGO_NET_RETRY=3"
set "RUSTUP_IO_THREADS=%JOBS%"
set "LOCAL_RUST=%~dp0.rust"
set "LOCAL_CARGO_BIN=%LOCAL_RUST%\cargo\bin"
set "OUT_DIR=%~dp0Build"

echo [1/5] Kiem tra cong cu Rust...
where cargo >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    where rustc >nul 2>&1
    if %ERRORLEVEL% EQU 0 (
        echo [OK] Da tim thay Rust he thong.
        goto :check_target
    )
)

if exist "%LOCAL_CARGO_BIN%\cargo.exe" (
    set "RUSTUP_HOME=%LOCAL_RUST%\rustup"
    set "CARGO_HOME=%LOCAL_RUST%\cargo"
    set "PATH=%LOCAL_CARGO_BIN%;%PATH%"
    echo [OK] Da tim thay Rust portable ke ben tool: %LOCAL_RUST%
    goto :check_target
)

echo [INFO] Chua co Rust. Dang tai rustup-init.exe ve ke ben tool (%JOBS% luong IO)...
if not exist "%LOCAL_RUST%" mkdir "%LOCAL_RUST%" >nul 2>&1
curl.exe -fsSL --retry 3 -o "%LOCAL_RUST%\rustup-init.exe" "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe"
if %ERRORLEVEL% NEQ 0 (
    echo [LOI] Khong the tai rustup-init.exe. Kiem tra ket noi mang.
    pause
    exit /b 1
)

set "RUSTUP_HOME=%LOCAL_RUST%\rustup"
set "CARGO_HOME=%LOCAL_RUST%\cargo"
echo [INFO] Dang cai dat Rust toolchain da luong (minimal, khong sua PATH he thong)...
"%LOCAL_RUST%\rustup-init.exe" -y --default-toolchain stable --default-host x86_64-pc-windows-msvc --no-modify-path --profile minimal
set "INIT_ERR=%ERRORLEVEL%"
del /f /q "%LOCAL_RUST%\rustup-init.exe" >nul 2>&1
if %INIT_ERR% NEQ 0 (
    echo [LOI] Cai dat Rust that bai!
    pause
    exit /b 1
)
set "PATH=%LOCAL_CARGO_BIN%;%PATH%"

:check_target
where rustup >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    rustup target list --installed | findstr /c:"x86_64-pc-windows-msvc" >nul 2>&1
    if %ERRORLEVEL% NEQ 0 (
        rustup target add x86_64-pc-windows-msvc
    )
)

echo [2/5] Kiem tra ffmpeg.exe ke ben build.bat...
set "HAS_FFMPEG=0"
if exist "%~dp0ffmpeg.exe" (
    for %%A in ("%~dp0ffmpeg.exe") do (
        if %%~zA GEQ 5000000 set "HAS_FFMPEG=1"
    )
)
if "%HAS_FFMPEG%"=="1" (
    echo [OK] Da tim thay ffmpeg.exe ke ben build.bat.
    goto :cleanup_old
)

echo [INFO] Chua co ffmpeg.exe ke ben. Dang tai ban moi nhat (ffmpeg-release-essentials)...
set "FF_TMP=%~dp0.ffmpeg_dl"
if exist "%FF_TMP%" rmdir /s /q "%FF_TMP%" >nul 2>&1
mkdir "%FF_TMP%" >nul 2>&1
curl.exe -fsSL --retry 3 -o "%FF_TMP%\ffmpeg.zip" "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
if %ERRORLEVEL% NEQ 0 (
    echo [LOI] Khong the tai ffmpeg-release-essentials.zip! Kiem tra ket noi mang.
    rmdir /s /q "%FF_TMP%" >nul 2>&1
    pause
    exit /b 1
)
echo [INFO] Dang giai nen ffmpeg.exe...
tar.exe -xf "%FF_TMP%\ffmpeg.zip" -C "%FF_TMP%"
if %ERRORLEVEL% NEQ 0 (
    echo [LOI] Giai nen ffmpeg.zip that bai!
    rmdir /s /q "%FF_TMP%" >nul 2>&1
    pause
    exit /b 1
)
for /r "%FF_TMP%" %%F in (ffmpeg.exe) do (
    if exist "%%F" copy /y "%%F" "%~dp0ffmpeg.exe" >nul 2>&1
)
rmdir /s /q "%FF_TMP%" >nul 2>&1
set "HAS_FFMPEG=0"
if exist "%~dp0ffmpeg.exe" (
    for %%A in ("%~dp0ffmpeg.exe") do (
        if %%~zA GEQ 5000000 set "HAS_FFMPEG=1"
    )
)
if "%HAS_FFMPEG%"=="0" (
    echo [LOI] Khong tim thay ffmpeg.exe hop le sau khi giai nen!
    pause
    exit /b 1
)
echo [OK] Da tai xong ffmpeg.exe ke ben build.bat!

:cleanup_old
echo [3/5] Dang don dep ban chay cu...
taskkill /f /im QuickSnipRecorder.exe >nul 2>&1

echo [4/5] Dang tai crates va build Release da luong (%JOBS% luong, ThinLTO, Strip)...
cargo build --release -j %JOBS% --target x86_64-pc-windows-msvc
if %ERRORLEVEL% NEQ 0 (
    echo [LOI] Qua trinh build that bai!
    pause
    exit /b 1
)

echo [5/5] Dang xuat 1 file EXE duy nhat (da gop FFmpeg) ra thu muc Build...
if not exist "%OUT_DIR%" mkdir "%OUT_DIR%" >nul 2>&1
if exist "%OUT_DIR%\ffmpeg.exe" del /f /q "%OUT_DIR%\ffmpeg.exe" >nul 2>&1
if exist "target\x86_64-pc-windows-msvc\release\QuickSnipRecorder.exe" (
    copy /y "target\x86_64-pc-windows-msvc\release\QuickSnipRecorder.exe" "%OUT_DIR%\QuickSnipRecorder.exe" >nul 2>&1
)

echo [HOAN TAT] File EXE doc lap duy nhat san sang tai: %OUT_DIR%\QuickSnipRecorder.exe
pause
