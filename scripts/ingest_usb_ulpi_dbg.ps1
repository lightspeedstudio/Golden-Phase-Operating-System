# Ingest #DBG_fe0da0# lines from a serial capture / pasted `usb enum` output.
# Always appends NDJSON to FullOS/debug-fe0da0.log; also POSTs to the debug ingest server.
param(
    [Parameter(Mandatory = $true)]
    [string]$CapturePath
)

$endpoint = "http://127.0.0.1:7496/ingest/94337083-96bc-4604-b82a-5bb23e9c8922"
$logPath = Join-Path (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path "debug-fe0da0.log"
if (-not (Test-Path -LiteralPath $CapturePath)) {
    Write-Error "Capture file not found: $CapturePath"
    exit 1
}

$n = 0
Get-Content -LiteralPath $CapturePath | ForEach-Object {
    $line = $_.Trim()
    if ($line -match '^#DBG_fe0da0#(.+)$') {
        $json = $Matches[1]
        Add-Content -LiteralPath $logPath -Value $json
        try {
            Invoke-RestMethod -Uri $endpoint -Method Post -ContentType "application/json" `
                -Headers @{ "X-Debug-Session-Id" = "fe0da0" } -Body $json | Out-Null
        } catch {
            Write-Warning "POST failed (file log still written): $_"
        }
        $n++
    }
}
Write-Host "done: $n debug line(s) -> $logPath"
