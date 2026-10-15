@echo off
setlocal EnableExtensions
cd /d "%~dp0.."

rem Fallback: raw binary loader at 0x80200000 (if kernel is linked there instead of 0x80000000).
rem Build raw image: rust-objcopy -O binary target\riscv64gc-unknown-none-elf\debug\kairos_boot target\riscv64gc-unknown-none-elf\debug\kairos_boot.bin

set "CARGO=%USERPROFILE%\.cargo\bin\cargo.exe"
set "ELF=%CD%\target\riscv64gc-unknown-none-elf\debug\kairos_boot"
set "BIN=%CD%\target\riscv64gc-unknown-none-elf\debug\kairos_boot.bin"
set "QEMU=C:\Program Files\qemu\qemu-system-riscv64.exe"
set "OBJCOPY=%USERPROFILE%\.rustup\toolchains\stable-x86_64-pc-windows-msvc\lib\rustlib\riscv64gc-unknown-none-elf\bin\llvm-objcopy.exe"

if not exist "%CARGO%" (
    echo Cargo not found: %CARGO%
    goto :fail
)
if not exist "%QEMU%" (
    echo QEMU not found: %QEMU%
    goto :fail
)

echo [1/3] Building kairos_boot...
"%CARGO%" build --bin kairos_boot --no-default-features --features baremetal --target riscv64gc-unknown-none-elf
if errorlevel 1 goto :fail

if not exist "%OBJCOPY%" (
    echo llvm-objcopy not found: %OBJCOPY%
    echo Install riscv64 target: rustup target add riscv64gc-unknown-none-elf
    goto :fail
)

echo [2/3] Creating raw binary...
"%OBJCOPY%" -O binary "%ELF%" "%BIN%"
if errorlevel 1 goto :fail

echo [3/3] Starting QEMU loader boot at 0x80200000...
"%QEMU%" -machine virt -m 128M -display none -monitor none -serial stdio -bios none -device loader,force-raw=on,file="%BIN%",addr=0x80200000 -device loader,cpu-num=0,addr=0x80200000
goto :end

:fail
echo Boot failed.
pause
exit /b 1

:end
pause
