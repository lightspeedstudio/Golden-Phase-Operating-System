# Gate-4 PhaseGuard QEMU evidence (baremetal + phaseguard-research)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP "kairos-target"
$elf = Join-Path $env:CARGO_TARGET_DIR "riscv64gc-unknown-none-elf\debug\kairos_boot"
$qemu = "C:\Program Files\qemu\qemu-system-riscv64.exe"
$port = 34571
$outLog = Join-Path $root "docs\research\PHASEGUARD_GATE4_QEMU_SERIAL.log"

if (-not (Test-Path $qemu)) {
    throw "QEMU not found at $qemu"
}

& $cargo build --bin kairos_boot --no-default-features --features "baremetal,phaseguard-research" --target riscv64gc-unknown-none-elf
if (-not (Test-Path $elf)) {
    throw "missing $elf"
}

$bytes = [IO.File]::ReadAllBytes($elf)
$entry = [BitConverter]::ToUInt64($bytes, 24)
Write-Host "ELF entry=0x$($entry.ToString('x')) features=baremetal,phaseguard-research"

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
    Start-Sleep -Milliseconds 400
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

    $deadline = (Get-Date).AddSeconds(12)
    while ((Get-Date) -lt $deadline) {
        Read-Available $stream $out
        if ($out.ToString() -match "kairos>\s*$") { break }
        Start-Sleep -Milliseconds 50
    }

    foreach ($cmd in @("ps", "phaseguard status", "phaseguard sample", "phaseguard sample", "phaseguard trace", "exit")) {
        $writer.WriteLine($cmd)
        Start-Sleep -Milliseconds 800
        $cmdDeadline = (Get-Date).AddSeconds(3)
        while ((Get-Date) -lt $cmdDeadline) {
            Read-Available $stream $out
            Start-Sleep -Milliseconds 30
        }
    }
    Start-Sleep -Milliseconds 400
    Read-Available $stream $out
    $tcp.Close()
}
finally {
    if (-not $qemuProc.HasExited) {
        Stop-Process -Id $qemuProc.Id -Force -ErrorAction SilentlyContinue
    }
}

$text = $out.ToString()
[IO.File]::WriteAllText($outLog, $text)
Write-Host "--- serial log written to $outLog ---"
Write-Host $text

$checks = @(
    "stateclock",
    "gpos>",
    "PhaseGuard status",
    "kairos_tick=",
    "[PhaseGuard]",
    "predicted=",
    "KairOS: halt"
)
$ok = $true
foreach ($needle in $checks) {
    if ($text -notmatch [regex]::Escape($needle)) {
        Write-Host "MISSING: $needle"
        $ok = $false
    }
}
# Lawful live stream: after boot + status/sample/trace commands, fault_count should be 0
# (status line: pass=N warn=0 fault=0). Squeeze previews do not mutate the observer.
if ($text -notmatch "warn=0 fault=0") {
    Write-Host "MISSING: warn=0 fault=0 (valid-trace false positives)"
    $ok = $false
}
if ($ok) {
    Write-Host "PHASEGUARD QEMU EVIDENCE OK"
    exit 0
}
Write-Host "PHASEGUARD QEMU EVIDENCE FAIL"
exit 1
