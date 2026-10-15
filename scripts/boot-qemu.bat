@echo off
setlocal EnableExtensions
cd /d "%~dp0.."

set "CARGO=%USERPROFILE%\.cargo\bin\cargo.exe"
set "CARGO_TARGET_DIR=%TEMP%\kairos-target"
set "ELF=%CARGO_TARGET_DIR%\riscv64gc-unknown-none-elf\debug\kairos_boot"
set "QEMU=C:\Program Files\qemu\qemu-system-riscv64.exe"

if not exist "%CARGO%" (
    echo Cargo not found: %CARGO%
    echo Install Rust from https://rustup.rs
    goto :fail
)
if not exist "%QEMU%" (
    echo QEMU not found: %QEMU%
    echo Install: choco install qemu -y   ^(admin CMD^)
    goto :fail
)

echo [1/2] Building kairos_boot...
"%CARGO%" build --bin kairos_boot --no-default-features --features baremetal --target riscv64gc-unknown-none-elf
if errorlevel 1 goto :fail
if not exist "%ELF%" (
    echo Missing boot image: %ELF%
    goto :fail
)

echo [2/2] Starting QEMU ^(M-mode, kernel linked at 0x80000000^)...
echo      Expected: KairOS boot splash, init pid 0, QEMU runtime, kairos^>
echo      Smoke script: scripts\qemu-smoke.bat
echo      Fallback if this fails: scripts\boot-qemu-loader.bat
echo      Quit: Ctrl+A then X
echo      Type at kairos^> — press Enter to run each command.
echo.
"%QEMU%" -machine virt -m 128M -nographic -bios none -kernel "%ELF%"
goto :end

:fail
echo.
echo Boot failed.
pause
exit /b 1

:end
pause
