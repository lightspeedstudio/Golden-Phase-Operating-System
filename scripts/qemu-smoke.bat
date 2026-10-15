@echo off
setlocal EnableExtensions
cd /d "%~dp0.."

set "CARGO=%USERPROFILE%\.cargo\bin\cargo.exe"
set "CARGO_TARGET_DIR=%TEMP%\kairos-target"
set "ELF=%CARGO_TARGET_DIR%\riscv64gc-unknown-none-elf\debug\kairos_boot"
set "QEMU=C:\Program Files\qemu\qemu-system-riscv64.exe"

echo === KairOS QEMU smoke (Layer 10) ===
echo.
echo [1/2] Build kairos_boot (RISC-V virt, M-mode, -bios none, link 0x80000000)
if not exist "%CARGO%" (
    echo Cargo not found: %CARGO%
    goto :fail
)
"%CARGO%" build --bin kairos_boot --no-default-features --features baremetal --target riscv64gc-unknown-none-elf
if errorlevel 1 goto :fail
if not exist "%ELF%" (
    echo Missing boot image: %ELF%
    goto :fail
)

echo [2/2] Start QEMU serial console
echo.
echo -------------------------------------------------------------------------------
echo   Boot splash + kairos^> shell appear BELOW this line (not above).
echo   Try: state, ps, mem, help, exit
echo   Quit QEMU: Ctrl+A then X
echo   Auto smoke: scripts\boot-qemu-smoke.ps1
echo -------------------------------------------------------------------------------
echo.

if not exist "%QEMU%" (
    echo QEMU not found: %QEMU%
    echo Install: choco install qemu -y   ^(admin CMD^)
    goto :fail
)

"%QEMU%" -machine virt -m 128M -nographic -bios none -kernel "%ELF%"
goto :end

:fail
echo.
echo Smoke setup failed.
pause
exit /b 1

:end
pause
