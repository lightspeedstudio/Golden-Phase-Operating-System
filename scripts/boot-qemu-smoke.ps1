# Non-interactive QEMU smoke test (M-mode, socket serial for Windows automation)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP "kairos-target"
$elf = Join-Path $env:CARGO_TARGET_DIR "riscv64gc-unknown-none-elf\debug\kairos_boot"
$qemu = "C:\Program Files\qemu\qemu-system-riscv64.exe"
$port = 34570

if (-not (Test-Path $qemu)) {
    throw "QEMU not found at $qemu"
}

& $cargo build --bin kairos_boot --no-default-features --features baremetal --target riscv64gc-unknown-none-elf
if (-not (Test-Path $elf)) {
    throw "missing $elf"
}

$bytes = [IO.File]::ReadAllBytes($elf)
$entry = [BitConverter]::ToUInt64($bytes, 24)
Write-Host "ELF entry=0x$($entry.ToString('x'))"

$qemuProc = Start-Process -FilePath $qemu -ArgumentList @(
    "-machine", "virt",
    "-m", "128M",
    "-display", "none",
    "-monitor", "none",
    "-bios", "none",
    "-kernel", $elf,
    "-chardev", "socket,id=ser,host=127.0.0.1,port=$port,server=on,wait=on,nodelay=on",
    "-serial", "chardev:ser"
) -PassThru -NoNewWindow

try {
    Start-Sleep -Milliseconds 300
    $tcp = New-Object Net.Sockets.TcpClient
    $tcp.Connect("127.0.0.1", $port)
    $stream = $tcp.GetStream()
    $writer = New-Object IO.StreamWriter($stream)
    $writer.AutoFlush = $true
    $out = New-Object System.Text.StringBuilder

    function Read-Available([IO.Stream]$s, [System.Text.StringBuilder]$buf) {
        while ($s.DataAvailable) {
            $b = $s.ReadByte()
            if ($b -ge 0) { [void]$buf.Append([char]$b) }
        }
    }

    $deadline = (Get-Date).AddSeconds(10)
    while ((Get-Date) -lt $deadline) {
        Read-Available $stream $out
        if ($out.ToString() -match "kairos>\s*$") { break }
        Start-Sleep -Milliseconds 50
    }

    foreach ($cmd in @("ps", "mem", "exit")) {
        $writer.WriteLine($cmd)
        Start-Sleep -Milliseconds 700
        $cmdDeadline = (Get-Date).AddSeconds(2)
        while ((Get-Date) -lt $cmdDeadline) {
            Read-Available $stream $out
            Start-Sleep -Milliseconds 30
        }
    }
    Start-Sleep -Milliseconds 300
    Read-Available $stream $out
    $tcp.Close()
}
finally {
    if (-not $qemuProc.HasExited) {
        Stop-Process -Id $qemuProc.Id -Force -ErrorAction SilentlyContinue
    }
}

$text = $out.ToString()
if ([string]::IsNullOrEmpty($text)) {
    Write-Host "SMOKE FAIL: empty serial log"
    exit 1
}
Write-Host "--- serial log ---"
Write-Host $text

$checks = @(
    "KairOS boot splash",
    "awakening",
    "stateclock",
    "init: pid 0",
    "kairos>",
    "running",
    "user",
    "KairOS: halt"
)
$ok = $true
foreach ($needle in $checks) {
    if ($text -notmatch [regex]::Escape($needle)) {
        Write-Host "MISSING: $needle"
        $ok = $false
    }
}
if ($ok) {
    Write-Host "SMOKE OK"
    exit 0
}
Write-Host "SMOKE FAIL"
exit 1
