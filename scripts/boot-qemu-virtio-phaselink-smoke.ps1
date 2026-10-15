# Two-node PhaseLink over VirtIO Ethernet smoke test
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP "kairos-target"
$previousRustFlags = $env:RUSTFLAGS
$env:RUSTFLAGS = "$previousRustFlags -Awarnings".Trim()
$elf = Join-Path $env:CARGO_TARGET_DIR "riscv64gc-unknown-none-elf\debug\kairos_boot"
$qemu = "C:\Program Files\qemu\qemu-system-riscv64.exe"
$linkPort = 40140
$portA = 40141
$portB = 40142

& $cargo build --bin kairos_boot --no-default-features --features baremetal --target riscv64gc-unknown-none-elf
if (-not (Test-Path $elf)) { throw "missing $elf" }
if (-not (Test-Path $qemu)) { throw "QEMU not found at $qemu" }

function Read-Available([IO.Stream]$s, [System.Text.StringBuilder]$buf) {
    while ($s.DataAvailable) {
        $b = $s.ReadByte()
        if ($b -ge 0) { [void]$buf.Append([char]$b) }
    }
}

function Wait-ForPrompt([IO.Stream]$stream, [System.Text.StringBuilder]$out, [int]$seconds = 15) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        Read-Available $stream $out
        if ($out.ToString() -match "kairos>\s*$") { return $true }
        Start-Sleep -Milliseconds 50
    }
    return $false
}

function Send-Commands([IO.StreamWriter]$writer, [IO.Stream]$stream, [System.Text.StringBuilder]$out, [string[]]$commands, [int]$pauseMs = 1500) {
    foreach ($cmd in $commands) {
        $writer.WriteLine($cmd)
        Start-Sleep -Milliseconds $pauseMs
        $cmdDeadline = (Get-Date).AddSeconds(4)
        while ((Get-Date) -lt $cmdDeadline) {
            Read-Available $stream $out
            Start-Sleep -Milliseconds 30
        }
    }
}

$commonArgs = @("-machine", "virt", "-m", "128M", "-display", "none", "-monitor", "none", "-bios", "none", "-kernel", $elf)
$nodeBProc = $null
$nodeAProc = $null
$tcpB = $null
$tcpA = $null

try {
    $chardevB = "socket,id=ser,host=127.0.0.1,port=$portB,server=on,wait=on,nodelay=on"
    $nodeBProc = Start-Process -FilePath $qemu -ArgumentList ($commonArgs + @(
        "-netdev", "socket,id=n0,listen=:$linkPort",
        "-device", "virtio-net-device,netdev=n0,mac=52:54:00:12:34:57",
        "-chardev", $chardevB,
        "-serial", "chardev:ser"
    )) -PassThru -NoNewWindow

    Start-Sleep -Milliseconds 500
    $tcpB = New-Object Net.Sockets.TcpClient
    $tcpB.Connect("127.0.0.1", $portB)
    $streamB = $tcpB.GetStream()
    $writerB = New-Object IO.StreamWriter($streamB)
    $writerB.AutoFlush = $true
    $outB = New-Object System.Text.StringBuilder
    if (-not (Wait-ForPrompt $streamB $outB)) { throw "node B did not reach prompt" }
    Send-Commands $writerB $streamB $outB @("net init", "phaselink init node-b node-a")

    $chardevA = "socket,id=ser,host=127.0.0.1,port=$portA,server=on,wait=on,nodelay=on"
    $nodeAProc = Start-Process -FilePath $qemu -ArgumentList ($commonArgs + @(
        "-netdev", "socket,id=n0,connect=127.0.0.1:$linkPort",
        "-device", "virtio-net-device,netdev=n0,mac=52:54:00:12:34:56",
        "-chardev", $chardevA,
        "-serial", "chardev:ser"
    )) -PassThru -NoNewWindow

    Start-Sleep -Milliseconds 500
    $tcpA = New-Object Net.Sockets.TcpClient
    $tcpA.Connect("127.0.0.1", $portA)
    $streamA = $tcpA.GetStream()
    $writerA = New-Object IO.StreamWriter($streamA)
    $writerA.AutoFlush = $true
    $outA = New-Object System.Text.StringBuilder
    if (-not (Wait-ForPrompt $streamA $outA)) { throw "node A did not reach prompt" }
    Send-Commands $writerA $streamA $outA @("net init", "phaselink init node-a node-b", "phaselink send node-b HELLO")

    Start-Sleep -Milliseconds 1000
    Send-Commands $writerB $streamB $outB @("phaselink recv") 2000

    Start-Sleep -Milliseconds 500
    Send-Commands $writerA $streamA $outA @("phaselink replay")

    Start-Sleep -Milliseconds 1000
    Send-Commands $writerB $streamB $outB @("phaselink recv", "phaselink status", "exit") 2000
    Send-Commands $writerA $streamA $outA @("exit")
    Read-Available $streamB $outB
    Read-Available $streamA $outA

    $nodeAText = $outA.ToString()
    $nodeBText = $outB.ToString()

    Write-Host "--- node A ---"
    Write-Host $nodeAText
    Write-Host "--- node B ---"
    Write-Host $nodeBText

    $ok = $true
    if ($nodeAText -notmatch "phaselink sent to node-b: HELLO") {
        Write-Host "MISSING: node A phaselink send"
        $ok = $false
    }
    if ($nodeAText -notmatch "phaselink replay sent") {
        Write-Host "MISSING: node A phaselink replay"
        $ok = $false
    }
    if ($nodeBText -notmatch "\[ RESULT  \] ACCEPTED") {
        Write-Host "MISSING: node B first recv ACCEPTED"
        $ok = $false
    }
    if ($nodeBText -notmatch "\[ PAYLOAD \] HELLO") {
        Write-Host "MISSING: node B payload HELLO"
        $ok = $false
    }
    if ($nodeBText -notmatch "replay detected") {
        Write-Host "MISSING: node B replay quarantine"
        $ok = $false
    }
    if ($nodeBText -notmatch "accepted frames: 1") {
        Write-Host "MISSING: node B accepted frames: 1"
        $ok = $false
    }
    if ($nodeBText -notmatch "quarantined frames: 1") {
        Write-Host "MISSING: node B quarantined frames: 1"
        $ok = $false
    }
    if ($ok) {
        Write-Host "PHASELINK ETH OK"
        exit 0
    }
    Write-Host "PHASELINK ETH FAIL"
    exit 1
}
finally {
    if ($tcpA) { $tcpA.Close() }
    if ($tcpB) { $tcpB.Close() }
    if ($nodeAProc -and -not $nodeAProc.HasExited) {
        Stop-Process -Id $nodeAProc.Id -Force -ErrorAction SilentlyContinue
    }
    if ($nodeBProc -and -not $nodeBProc.HasExited) {
        Stop-Process -Id $nodeBProc.Id -Force -ErrorAction SilentlyContinue
    }
    $env:RUSTFLAGS = $previousRustFlags
}
