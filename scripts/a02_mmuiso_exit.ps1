# A02: MMU COM12 harness exit code. Prompt-return is not a PASS.
# Does not open COM. Does not classify matrix order (that is A03).

function Get-MmuisoHarnessExitCode {
    param(
        [Parameter(Mandatory = $true)][bool]$CommandsGotPrompt,
        [Parameter(Mandatory = $true)][int]$MatrixPassCount
    )
    if (-not $CommandsGotPrompt) {
        return 4
    }
    if ($MatrixPassCount -le 0) {
        return 5
    }
    return 0
}

function Test-A02MmuisoExit {
    $old = {
        param([bool]$Ok, [int]$Pass)
        if ($Ok) { return 0 } else { return 4 }
    }

    # Negative: old implementation exits 0 with pass=0.
    $negOld = & $old $true 0
    $negNew = Get-MmuisoHarnessExitCode -CommandsGotPrompt $true -MatrixPassCount 0
    if ($negOld -ne 0) { throw "old rule should exit 0 on prompt+pass=0" }
    if ($negNew -eq 0) { throw "A02: prompt+pass=0 must not exit 0" }
    if ($negNew -ne 5) { throw "A02: expected exit 5, got $negNew" }

    # Positive: at least one matrix PASS line and prompts returned.
    $pos = Get-MmuisoHarnessExitCode -CommandsGotPrompt $true -MatrixPassCount 1
    if ($pos -ne 0) { throw "A02: prompt+pass=1 must exit 0, got $pos" }

    # Boundary: pass=0 with fails still not exit 0 (fail count is not this finding).
    $bound = Get-MmuisoHarnessExitCode -CommandsGotPrompt $true -MatrixPassCount 0
    if ($bound -eq 0) { throw "A02: pass=0 must not exit 0" }

    # Regression: command timeout still exit 4 even if a stale pass count is nonzero.
    $to = Get-MmuisoHarnessExitCode -CommandsGotPrompt $false -MatrixPassCount 8
    if ($to -ne 4) { throw "A02: timeout must stay exit 4, got $to" }

    Write-Host "a02_mmuiso_exit selftest: ok"
}

if ($args -contains "--selftest") {
    Test-A02MmuisoExit
    exit 0
}
