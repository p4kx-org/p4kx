$ErrorActionPreference = 'Stop'

function Get-P4kxMsiFinishProbeMilliseconds {
    param([DateTimeOffset]$Deadline, [Diagnostics.Stopwatch]$Clock, [double]$Budget)
    $wall=($Deadline - [DateTimeOffset]::UtcNow).TotalMilliseconds
    return [int][Math]::Max(0,[Math]::Floor([Math]::Min($wall,$Budget - $Clock.Elapsed.TotalMilliseconds)))
}

function Wait-P4kxMsiFinishReceiptFile {
    param([string]$ReceiptPath, [DateTimeOffset]$Cutoff, [Diagnostics.Stopwatch]$Clock, [double]$Budget)
    while ($true) {
        $remaining = Get-P4kxMsiFinishProbeMilliseconds $Cutoff $Clock $Budget
        if ($remaining -eq 0) { return $false }
        if ([IO.File]::Exists($ReceiptPath)) { return $true }
        Start-Sleep -Milliseconds ([Math]::Min(100, $remaining))
    }
}
