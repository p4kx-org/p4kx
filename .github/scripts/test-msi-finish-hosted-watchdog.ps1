param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'msi-observer-finish-watchdog.ps1')

$root = [IO.Path]::GetFullPath($Directory)
if (-not $IsWindows -or [IO.Path]::Exists($root)) { throw 'The hosted Finish watchdog requires a fresh Windows directory.' }
$null = [IO.Directory]::CreateDirectory($root)
$worker = Join-Path $root 'fixture worker.ps1'
$workerBody = @'
param([string]$Directory, [string]$CollectionCutoffUtc)
$ErrorActionPreference = 'Stop'
[IO.File]::WriteAllText((Join-Path $Directory 'worker.pid'), [string]$PID)
[Console]::WriteLine($CollectionCutoffUtc)
[Console]::Error.WriteLine('fixture diagnostic')
switch (Split-Path -Leaf $Directory) {
    'normal' { exit 0 }
    'failure' { exit 7 }
    'hung' { [Threading.Thread]::Sleep(30000) }
    'digest' {
        . (Join-Path 'FIXTURE_SCRIPT_DIRECTORY' 'msi-observer-finish-control.ps1')
        function Get-P4kxDetachedReceiptDigest { [Threading.Thread]::Sleep(30000) }
        $private = Join-Path $Directory 'finish-supervision'
        [IO.File]::WriteAllText((Join-Path $private 'result.json'), '{}')
        $cutoff = ConvertFrom-P4kxMsiBudgetTimestamp $CollectionCutoffUtc
        Wait-P4kxMsiFinishReceipt @{directory=$private;collection_cutoff=$cutoff;digest=('a'*64)} @{} -Hosted
    }
    'missing' {
        . (Join-Path 'FIXTURE_SCRIPT_DIRECTORY' 'msi-observer-finish-control.ps1')
        $private = Join-Path $Directory 'finish-supervision'
        $cutoff = ConvertFrom-P4kxMsiBudgetTimestamp $CollectionCutoffUtc
        Wait-P4kxMsiFinishReceipt @{directory=$private;collection_cutoff=$cutoff;digest=('a'*64)} @{} -Hosted
    }
    'late-exit' {
        $cutoff = [DateTimeOffset]::Parse($CollectionCutoffUtc)
        $remaining = ($cutoff - [DateTimeOffset]::UtcNow).TotalMilliseconds
        [Threading.Thread]::Sleep([int][Math]::Max(0, [Math]::Ceiling($remaining) + 25))
        [IO.File]::WriteAllText((Join-Path $Directory 'finish-supervision/wait-result.json'), [DateTimeOffset]::UtcNow.ToString('o'))
        exit 0
    }
    'after-receipt' {
        [IO.File]::WriteAllText((Join-Path $Directory 'finish-supervision/wait-result.json'), 'fixture observation')
        [Threading.Thread]::Sleep(30000)
    }
    default { throw 'Unexpected hosted Finish watchdog case.' }
}
'@
$workerBody.Replace('FIXTURE_SCRIPT_DIRECTORY', $PSScriptRoot.Replace("'","''")) |
    Set-Content -LiteralPath $worker -Encoding utf8NoBOM

$cases = @(
    @{name='normal';code=0}, @{name='failure';code=7}, @{name='hung';code=$null},
    @{name='digest';code=$null}, @{name='after-receipt';code=$null},
    @{name='missing';code=$null}, @{name='late-exit';code=$null}
)
foreach ($case in $cases) {
    $folder = Join-Path $root $case.name
    $logs = [IO.Directory]::CreateDirectory((Join-Path $folder 'finish-supervision')).FullName
    $cutoff = [DateTimeOffset]::UtcNow.AddSeconds(5).ToString('o')
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $actual = $null
    try {
        $actual = Invoke-P4kxMsiHostedFinishWatchdog -Directory $folder -CollectionCutoffUtc $cutoff -WorkerScript $worker
    } catch {
        if ($null -ne $case.code -or $_.Exception.Message -cne 'Hosted Finish observation exceeded its original collection deadline.') { throw }
    }
    if ($actual -ne $case.code -or $timer.Elapsed.TotalSeconds -ge 12) {
        throw 'Hosted Finish watchdog lost the exact exit status or exceeded its bounded cleanup.'
    }
    $stdout = [IO.File]::ReadAllText((Join-Path $logs 'host-stdout.log')).Trim()
    $stderr = [IO.File]::ReadAllText((Join-Path $logs 'host-stderr.log')).Trim()
    $workerPid = [int][IO.File]::ReadAllText((Join-Path $folder 'worker.pid'))
    if (($stdout -split '\r?\n')[0] -cne $cutoff -or ($stderr -split '\r?\n')[0] -cne 'fixture diagnostic' -or (Get-Process -Id $workerPid -ErrorAction SilentlyContinue)) {
        throw 'Hosted Finish watchdog lost private diagnostics, changed its cutoff or left its owned worker alive.'
    }
    $receipt = Join-Path $logs 'wait-result.json'
    if ($case.name -ceq 'digest' -and
        (-not [IO.File]::Exists((Join-Path $logs 'finishWaitDigest-begin.json')) -or [IO.File]::Exists($receipt))) {
        throw 'The stalled canonical digest did not retain its stage diagnostic or published an observation.'
    }
    if ($case.name -ceq 'after-receipt' -and -not [IO.File]::Exists($receipt)) {
        throw 'The stalled worker did not reach its post-publication boundary.'
    }
    if ($case.name -ceq 'missing') {
        if (-not [IO.File]::Exists($receipt)) { throw 'Missing result lost its original negative wait receipt.' }
        $observation = [IO.File]::ReadAllText($receipt) | ConvertFrom-Json -AsHashtable -DateKind String
        if ($null -ne $observation.result_sha256 -or $observation.deadline_utc -cne $cutoff -or
            $observation.mutation_authority -ne $false) { throw 'Missing result changed the original observation or deadline.' }
    }
    if ($case.name -ceq 'late-exit') {
        if (-not [IO.File]::Exists($receipt) -or
            (ConvertFrom-P4kxMsiBudgetTimestamp ([IO.File]::ReadAllText($receipt))) -le (ConvertFrom-P4kxMsiBudgetTimestamp $cutoff)) {
            throw 'The late-exit fixture did not reach its zero exit after the original deadline.'
        }
    }
}
'PASS: Hosted Finish watchdog retained exact statuses and diagnostics and reaped stalled workers before, during and after receipt observation.'
