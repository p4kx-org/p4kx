param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-path-probe-run.ps1')
$root = [IO.Path]::GetFullPath($Directory)
if (Test-Path -LiteralPath $root) { throw 'PATH probe contract directory must be new.' }
New-Item -ItemType Directory -Path $root | Out-Null
$fixtures = @(Get-P4kxPathFixtureCases)
$expected = @(
    @('expand_plain','ExpandString','%USERPROFILE%\.dotnet\tools;C:\P4kxSynthetic\foreign'),
    @('expand_leading','ExpandString',';%USERPROFILE%\.dotnet\tools;C:\P4kxSynthetic\foreign'),
    @('expand_trailing','ExpandString','%USERPROFILE%\.dotnet\tools;C:\P4kxSynthetic\foreign;'),
    @('expand_doubled','ExpandString',';;%USERPROFILE%\.dotnet\tools;;C:\P4kxSynthetic\foreign;;'),
    @('string_plain','String','C:\P4kxSynthetic\first;C:\P4kxSynthetic\foreign'),
    @('string_empty','String',''), @('expand_empty','ExpandString',''), @('absent',$null,$null)
)
if ($fixtures.Count -ne $expected.Count) { throw 'Adversarial PATH fixtures were omitted.' }
for ($index = 0; $index -lt $expected.Count; $index++) {
    $fixture = $fixtures[$index]
    Assert-P4kxPathFixtureTarget $fixture.state
    if ($fixture.name -cne $expected[$index][0] -or
        $fixture.state.registry_kind -cne $expected[$index][1] -or
        $fixture.state.raw_value -cne $expected[$index][2] -or
        $fixture.state.exists -cne ($fixture.name -cne 'absent')) { throw 'A raw PATH fixture was normalised or changed kind/existence.' }
}
foreach ($invalid in @(
    @{exists='true';registry_name='Path';registry_kind='String';raw_value='x'},
    @{exists=$true;registry_name='Other';registry_kind='String';raw_value='x'},
    @{exists=$true;registry_name='Path';registry_kind='MultiString';raw_value='x'},
    @{exists=$true;registry_name='Path';registry_kind='String';raw_value=$null},
    @{exists=$false;registry_name='Other';registry_kind=$null;raw_value=$null},
    @{exists=$false;registry_name=$null;registry_kind='String';raw_value=''}
)) {
    $rejected = $false
    try { Assert-P4kxPathFixtureTarget ([pscustomobject]$invalid) } catch { $rejected = $true }
    if (-not $rejected) { throw 'A foreign or invalid PATH fixture was accepted.' }
}
if ($env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or -not $IsWindows) {
    $rejected = $false
    try { Assert-P4kxHostedPathFixture } catch { $rejected = $true }
    if (-not $rejected) { throw 'The PATH writer could run outside its disposable hosted-runner boundary.' }
}

function Copy-TestPathState {
    param($State)
    return $State | ConvertTo-Json | ConvertFrom-Json
}

function Get-P4kxUserPathState { return Copy-TestPathState $script:state }

function Get-P4kxInstalledProducts {
    param([string]$Upgrade)
    if ($script:installed) { return '{11111111-2222-4333-8444-555555555555}' }
}

function Assert-P4kxInstallationOwnershipAbsent {
    param([string]$Upgrade, [string]$Folder, [string]$Shortcut)
    if ($script:installed -or $script:cleanupFails) { throw 'synthetic ownership cleanup failure' }
}

function Assert-P4kxLegacyOwnershipAbsent { param($Context) }

function Assert-P4kxInstalledProduct {
    param($Metadata, [string]$Folder, [string]$Shortcut)
    if (-not $script:installed) { throw 'Synthetic installation did not occur.' }
}

function Set-P4kxPathFixture {
    param($Context, $Target, $Expected, [string]$Log)
    Assert-P4kxPathFixtureTarget $Target
    Assert-P4kxInstallationOwnershipAbsent $Context.upgrade $Context.folder $Context.shortcut
    Assert-P4kxPathRestored $Expected $script:state
    $before = Get-P4kxUserPathState
    $script:state = Copy-TestPathState $Target
    $script:state | Add-Member -NotePropertyName user_api_value -NotePropertyValue $Target.raw_value -Force
    Write-P4kxPathTransition $before $script:state $Log
    $script:resets++
    return Get-P4kxUserPathState
}

function Invoke-P4kxMsiexec {
    param([string[]]$Arguments, [string]$Log, [int]$ExpectedCode=0)
    if ($Arguments[0] -ceq '/i') { $script:installed = $true; $script:installCount++; return }
    if ($Arguments[0] -cne '/x') { throw 'Unexpected synthetic MSI operation.' }
    $script:installed = $false
    if ($script:state.exists) {
        switch ($script:mutation) {
            'separators' { $script:state.raw_value = $script:state.raw_value.Trim(';') }
            'kind' { $script:state.registry_kind = 'String' }
            'legacy' { $script:state.raw_value = $script:state.raw_value.TrimEnd(';') }
        }
        $script:state.user_api_value = $script:state.raw_value
    }
}

$baseline = [pscustomobject]@{
    exists=$true;registry_name='Path';registry_kind='ExpandString'
    raw_value='%USERPROFILE%\host-original;';user_api_value='%USERPROFILE%\host-original;'
}
$context = [pscustomobject]@{
    msi='synthetic.msi';metadata=[pscustomobject]@{product='{11111111-2222-4333-8444-555555555555}'}
    upgrade='090C8F25-8543-42F0-B40A-1514653149F9';folder='synthetic folder';shortcut='synthetic shortcut';legacy_keys=@()
}
$script:cleanupFails = $false
foreach ($mutation in @('none','separators','kind')) {
    $context | Add-Member -NotePropertyName logs -NotePropertyValue (Join-Path $root $mutation) -Force
    $script:state = Copy-TestPathState $baseline
    $script:installed = $false; $script:resets = 0; $script:installCount = 0; $script:mutation = $mutation
    $results = @(Invoke-P4kxAdversarialPathCases $context $baseline)
    if ($results.Count -ne 8 -or $script:installCount -ne 8 -or $script:resets -ne 16) { throw 'A failing PATH case prevented run-all or its separate fixture reset.' }
    $failed = @($results | Where-Object { $_.result.outcome -ceq 'failed' })
    $count = switch ($mutation) { 'none' { 0 } 'separators' { 3 } 'kind' { 5 } }
    if ($failed.Count -ne $count) { throw 'Exact raw PATH or registry-kind regression was accepted.' }
    foreach ($result in $results) {
        if ($result.result.case -ceq 'absent' -and $result.result.before.exists) { throw 'Absent PATH was substituted with an empty string.' }
        if ($result.result.case -ceq 'expand_empty' -and
            (-not $result.result.before.exists -or $result.result.before.registry_kind -cne 'ExpandString')) { throw 'An empty typed PATH was dropped.' }
    }
    Assert-P4kxPathRestored $baseline (Get-P4kxUserPathState)
}
$context.logs = Join-Path $root 'cleanup-failure'
$script:cleanupFails = $true
$resetCountBeforeFailure = $script:resets
$caught = $null
try { Invoke-P4kxPathProbeCase $context $baseline 'failure' { throw 'synthetic original failure' } | Out-Null }
catch { $caught = $_.Exception }
if ($caught -isnot [AggregateException] -or $caught.InnerExceptions.Count -ne 2 -or
    $caught.InnerExceptions[0].Message -cne 'synthetic original failure' -or
    $caught.InnerExceptions[1].Message -cne 'synthetic ownership cleanup failure' -or $script:resets -ne $resetCountBeforeFailure) {
    throw 'Cleanup swallowed the original failure or reset PATH while ownership was unresolved.'
}
$script:cleanupFails = $false
$context.logs = Join-Path $root 'evidence-failure'
$script:mutation = 'none'
$resetCountBeforeEvidence = $script:resets
$caught = $null
try {
    Invoke-P4kxPathProbeCase $context $baseline 'failure' {
        param($directory)
        $script:installed = $true
        $script:state.raw_value = 'synthetic changed state'
        New-Item -ItemType Directory -Path (Join-Path $directory 'case.json') | Out-Null
    } | Out-Null
}
catch { $caught = $_.Exception }
if ($null -eq $caught) { throw 'Synthetic evidence write did not fail.' }
if ($script:installed) { throw 'Evidence failure skipped ownership cleanup.' }
if ($script:resets -ne ($resetCountBeforeEvidence + 1)) { throw 'Evidence failure skipped its separate fixture reset.' }
Assert-P4kxPathRestored $baseline (Get-P4kxUserPathState)
function Get-P4kxMsiMetadata {
    param([string]$Path)
    return [pscustomobject]@{
        template='x64;1033';version='0.2.0';upgrade=$context.upgrade
        product='{11111111-2222-4333-8444-555555555555}'
        registry=@(,@('1','Software\KRT\P4K-Extractor','Application'))
    }
}
$context.logs = Join-Path $root 'legacy'
$script:mutation = 'legacy'; $script:state = Copy-TestPathState $baseline
$rest = Get-P4kxObservedLegacyRest $context 'synthetic-previous.msi'
if ((Get-P4kxPathDifference $baseline $rest) -cne 'rawValueChanged' -or $rest.raw_value.EndsWith(';')) { throw 'Legacy cleanup loss was relabelled as exact preservation.' }
$transition = Get-Content (Join-Path $context.logs 'legacy-calibration/legacy-prior-mutation.path.json') -Raw | ConvertFrom-Json
if ($transition.before.raw_value -cne $baseline.raw_value -or $transition.after.raw_value -cne $rest.raw_value) { throw 'Legacy calibration lost its observed before/cleanup-rest boundary.' }
. (Join-Path $PSScriptRoot 'test-msi-path-probe-run.ps1') -Context $context -Baseline $baseline -Directory $root
'PASS: eight exact raw/kind/existence fixtures; guarded writer; run-all mutation rejection; explicit resets; original+cleanup failure; honest legacy cleanup boundary. All installer and registry operations mocked.'
