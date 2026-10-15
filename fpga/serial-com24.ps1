# Open USB serial console to iCEBreaker (default COM24 @ 115200)
param(
    [string]$Port = "COM24",
    [int]$Baud = 115200,
    [int]$Seconds = 0
)
$ErrorActionPreference = "Stop"
$sp = New-Object System.IO.Ports.SerialPort $Port, $Baud, 'None', 8, 'One'
$sp.ReadTimeout = 500
$sp.NewLine = "`n"
Write-Host "Opening $Port @ $Baud - Ctrl+C to exit"
$sp.Open()
try {
    $end = if ($Seconds -gt 0) { [DateTime]::UtcNow.AddSeconds($Seconds) } else { [DateTime]::MaxValue }
    while ([DateTime]::UtcNow -lt $end) {
        try {
            $chunk = $sp.ReadExisting()
            if ($chunk) { Write-Host -NoNewline $chunk }
            else { Start-Sleep -Milliseconds 30 }
        } catch [TimeoutException] { }
    }
} finally {
    if ($sp.IsOpen) { $sp.Close() }
}
