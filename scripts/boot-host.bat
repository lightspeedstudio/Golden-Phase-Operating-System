@echo off
setlocal EnableExtensions
cd /d "%~dp0.."

set "CARGO=%USERPROFILE%\.cargo\bin\cargo.exe"
if not exist "%CARGO%" (
    echo Cargo not found: %CARGO%
    pause
    exit /b 1
)

echo KairOS hosted runtime
echo Commands: ps ^| mem ^| run hello ^| run hello_asm ^| log ^| exit
echo.
"%CARGO%" run --bin kairos_host
pause
