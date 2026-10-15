# Interactive serial terminal for the PZ7020 Golden Phase OS console.
#
#   powershell -ExecutionPolicy Bypass -File scripts\serial_com12.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\serial_com12.ps1 -Port COM7
#
# Type commands and press Enter to send; the board's output streams back live.
# Press Ctrl+C to quit. Only ONE program can hold the port -- close other
# captures/monitors first.

param(
    [string]$Port = "COM12",
    [int]$Baud = 115200,
    [string]$Transcript = ""
)

# Render the OS's UTF-8 box-drawing splash correctly (else it shows as '?').
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch {}
try { chcp 65001 > $null } catch {}

$p = New-Object System.IO.Ports.SerialPort $Port, $Baud, None, 8, One
$p.Encoding = [System.Text.Encoding]::UTF8
$p.ReadTimeout = 50
$p.WriteTimeout = 500
$p.NewLine = "`r"
# Board rule: DTR and RTS must stay low. Raising either on open can reset or
# glitch the PZ7020. .NET defaults these to false -- set them anyway so the
# guarantee is explicit and survives any future edit.
$p.DtrEnable = $false
$p.RtsEnable = $false
$p.Handshake = [System.IO.Ports.Handshake]::None
try { $p.Open() } catch { Write-Host "Cannot open $Port : $($_.Exception.Message)" -ForegroundColor Red; return }

Write-Host "=== $Port @ $Baud 8-N-1  --  type + Enter to send, Ctrl+C to quit ===" -ForegroundColor Cyan
Write-Host "(power-cycle the board to see it boot; try: help, g64 info, g64 atlas)" -ForegroundColor DarkGray

try {
    while ($true) {
        # stream incoming bytes to the screen
        try {
            $in = $p.ReadExisting()
            if ($in) {
                Write-Host -NoNewline $in
                if ($Transcript) { [System.IO.File]::AppendAllText($Transcript, $in, [System.Text.Encoding]::UTF8) }
            }
        } catch {}

        # forward local keystrokes to the board
        while ([Console]::KeyAvailable) {
            $k = [Console]::ReadKey($true)
            switch ($k.Key) {
                'Enter'     { $p.Write("`r"); Write-Host "" }
                'Backspace' { $p.Write([char]8) }
                default     { if ($k.KeyChar) { $p.Write([string]$k.KeyChar) } }
            }
        }
        Start-Sleep -Milliseconds 15
    }
}
finally {
    $p.Close()
    Write-Host "`n[closed $Port]" -ForegroundColor DarkGray
}
