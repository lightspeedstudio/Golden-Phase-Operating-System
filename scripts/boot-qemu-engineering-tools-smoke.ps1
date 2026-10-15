# Engineering tools QEMU smoke test
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP "kairos-target"
$previousRustFlags = $env:RUSTFLAGS
$env:RUSTFLAGS = "$previousRustFlags -Awarnings".Trim()
$elf = Join-Path $env:CARGO_TARGET_DIR "riscv64gc-unknown-none-elf\debug\kairos_boot"
$qemu = "C:\Program Files\qemu\qemu-system-riscv64.exe"
$port = 40210

& $cargo build --bin kairos_boot --no-default-features --features baremetal --target riscv64gc-unknown-none-elf
if (-not (Test-Path $elf)) { throw "missing $elf" }
if (-not (Test-Path $qemu)) { throw "QEMU not found at $qemu" }

function Read-Available([IO.Stream]$s, [System.Text.StringBuilder]$buf) {
    while ($s.DataAvailable) {
        $b = $s.ReadByte()
        if ($b -ge 0) { [void]$buf.Append([char]$b) }
    }
}

function Wait-ForPrompt([IO.Stream]$stream, [System.Text.StringBuilder]$out, [int]$seconds = 20) {
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
        $deadline = (Get-Date).AddSeconds(6)
        while ((Get-Date) -lt $deadline) {
            Read-Available $stream $out
            Start-Sleep -Milliseconds 30
        }
    }
}

$proc = $null
$tcp = $null
$out = New-Object System.Text.StringBuilder
try {
    $chardev = "socket,id=ser,host=127.0.0.1,port=$port,server=on,wait=on,nodelay=on"
    $proc = Start-Process -FilePath $qemu -ArgumentList @(
        "-machine", "virt", "-m", "128M", "-display", "none", "-monitor", "none", "-bios", "none",
        "-kernel", $elf, "-chardev", $chardev, "-serial", "chardev:ser"
    ) -PassThru -NoNewWindow

    Start-Sleep -Milliseconds 500
    $tcp = New-Object Net.Sockets.TcpClient
    $tcp.Connect("127.0.0.1", $port)
    $stream = $tcp.GetStream()
    $writer = New-Object IO.StreamWriter($stream)
    $writer.AutoFlush = $true
    if (-not (Wait-ForPrompt $stream $out)) { throw "prompt timeout" }

    Send-Commands $writer $stream $out @(
        "help",
        "version",
        "pwd",
        "ls /",
        "mkdir /tmp/tools-test",
        "touch /tmp/tools-test/a.txt",
        "write /tmp/tools-test/a.txt smoke-test",
        "cat /tmp/tools-test/a.txt",
        "grep smoke /tmp/tools-test/a.txt",
        "stat /tmp/tools-test/a.txt",
        "checksum /tmp/tools-test/a.txt",
        "ps",
        "mem status",
        "clock status",
        "dev list",
        "net status",
        "phase status",
        "statechain status",
        "phi-stability status",
        "rm /tmp/tools-test/a.txt"
    )

    $text = $out.ToString()
    $markers = @(
        "SHELL TOOLS OK",
        "FILE TOOLS OK",
        "KAIROS ENGINEERING TOOLS SMOKE OK"
    )
    foreach ($m in $markers) {
        if ($text -notmatch [regex]::Escape($m)) {
            Write-Host $text
            throw "missing marker: $m"
        }
    }
    Write-Host "KAIROS ENGINEERING TOOLS SMOKE OK"
}
finally {
    if ($tcp) { $tcp.Close() }
    if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
}

& $cargo test --test engineering_tools_tests 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) { throw "engineering_tools_tests failed" }
