# Capture isolation boot breadcrumbs on COM12 into debug-638b5a.log
param(
    [string]$Port = "COM12",
    [int]$Baud = 115200,
    [int]$WaitSec = 25,
    [string]$LogPath = "C:\Users\richa\OneDrive\Desktop\FullOS\debug-638b5a.log"
)

$ErrorActionPreference = "Stop"
$sp = New-Object System.IO.Ports.SerialPort $Port, $Baud, ([System.IO.Ports.Parity]::None), 8, ([System.IO.Ports.StopBits]::One)
$sp.ReadTimeout = 200
$sp.DtrEnable = $false
$sp.RtsEnable = $false
$sb = New-Object System.Text.StringBuilder

function Write-Dbg([string]$HypothesisId, [string]$Message, [hashtable]$Data) {
    $obj = [ordered]@{
        sessionId = "638b5a"
        runId = "post-fix-1"
        hypothesisId = $HypothesisId
        location = "capture_mmuiso_bootprobe.ps1"
        message = $Message
        data = $Data
        timestamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    }
    ($obj | ConvertTo-Json -Compress -Depth 6) | Add-Content -Path $LogPath -Encoding UTF8
}

try {
    $sp.Open()
} catch {
    Write-Dbg "A" "com12_open_failed" @{ err = $_.Exception.Message }
    throw
}

Write-Host "COM12 open DTR/RTS off. PRESS PZ7020 RESET now. Capturing ${WaitSec}s..."
$deadline = (Get-Date).AddSeconds($WaitSec)
while ((Get-Date) -lt $deadline) {
    try {
        $c = $sp.ReadExisting()
        if ($c) {
            [void]$sb.Append($c)
            Write-Host -NoNewline $c
        }
    } catch {}
    Start-Sleep -Milliseconds 30
}
$sp.Close()
$raw = $sb.ToString()
$hex = ([System.Text.Encoding]::UTF8.GetBytes($raw) | ForEach-Object { $_.ToString("X2") }) -join ""
Write-Dbg "A" "uart_capture" @{
    len = $raw.Length
    raw = $raw.Substring(0, [Math]::Min(400, $raw.Length))
    has_caret1 = $raw.Contains("^1")
    has_caret2 = $raw.Contains("^2")
    has_caret3 = $raw.Contains("^3")
    has_caret4 = $raw.Contains("^4")
    has_caret5 = $raw.Contains("^5")
    has_caret6 = $raw.Contains("^6")
    has_Dbang = $raw.Contains("D!")
    has_P = $raw.Contains("P")
    has_panic = $raw.Contains("panic")
    has_fsbl = $raw.ToLower().Contains("fsbl")
    hex_prefix = if ($hex.Length -gt 128) { $hex.Substring(0,128) } else { $hex }
}
Write-Host "`n--- captured $($raw.Length) chars ---"
Write-Host "wrote $LogPath"
