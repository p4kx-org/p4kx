param([string]$Repository=(Split-Path (Split-Path $PSScriptRoot -Parent) -Parent))
$ErrorActionPreference='Stop'

function Assert-ChainWorkflow([bool]$Condition,[string]$Message) {
    if(-not $Condition){throw $Message}
}
function Get-ChainWorkflowStep([string]$Workflow,[string]$Name) {
    $matches=[regex]::Matches($Workflow,'(?ms)^      - name: '+[regex]::Escape($Name)+'\r?\n.*?(?=^      - |\z)')
    if($matches.Count -ne 1){throw "Expected one actual workflow step: $Name"}
    return $matches[0].Value
}
$release=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/workflows/release-windows.yml')
$prepare=Get-ChainWorkflowStep $release 'Prepare installer inputs and the independent step allowance'
$installer=Get-ChainWorkflowStep $release 'Installer metadata, upgrade, downgrade, rollback, CPU replacement and preservation'
$early=Get-ChainWorkflowStep $release 'Preserve prepared allowance, inputs and executed parent controls'
Assert-ChainWorkflow ($release.IndexOf($prepare,[StringComparison]::Ordinal) -lt $release.IndexOf($early,[StringComparison]::Ordinal) -and
    $release.IndexOf($early,[StringComparison]::Ordinal) -lt $release.IndexOf($installer,[StringComparison]::Ordinal)) 'Actual prepared receipt must upload after preparation and before native installer work.'
Assert-ChainWorkflow ($early -cmatch '(?m)^        timeout-minutes: 1\r?$' -and $early.Contains('installer-check/feedback/*.json') -and
    $early.Contains('contracts/msi-feedback-parent/**/*.jsonl') -and $early.Contains('contracts/msi-feedback-parent/**/*.log') -and
    $early -cmatch '(?m)^          if-no-files-found: error\r?$') 'Actual pre-risk upload lacks bounded original prepared and parent-control evidence.'
Assert-ChainWorkflow (-not ($early -match '(?m)^        (if|continue-on-error):')) 'Actual pre-risk evidence failure must refuse installer admission.'
Assert-ChainWorkflow ($prepare.Contains('Write-P4kxMsiDiagnosticInputs $parameters $feedback') -and
    $prepare.Contains("'msi-invocation.json'") -and $installer.Contains("'msi-invocation.json'") -and
    $installer.Contains('ConvertFrom-Json -AsHashtable') -and $installer.Contains('check-msi.ps1 @parameters') -and
    -not $early.Contains('msi-invocation.json')) 'Prepared MSI input hashes must upload before the same runner-local invocation is used, without publishing its paths.'

$chain=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/workflows/msi-feedback-chain-probe.yml')
$step=Get-ChainWorkflowStep $chain 'Dynamic parent and Portal fixture'
$prepared=Get-ChainWorkflowStep $chain 'Prepare original dynamic chain allowance'
$uploaded=Get-ChainWorkflowStep $chain 'Preserve original prepared chain receipt'
Assert-ChainWorkflow ($prepared.Contains('id: msi-budget') -and $step -cmatch '(?m)^        timeout-minutes: \$\{\{ fromJSON\(steps\.msi-budget\.outputs\.minutes\) \}\}\r?$') 'Native chain bypasses actual prepared output and dynamic runner allowance.'
Assert-ChainWorkflow ($chain.IndexOf($prepared,[StringComparison]::Ordinal) -lt $chain.IndexOf($uploaded,[StringComparison]::Ordinal) -and
    $chain.IndexOf($uploaded,[StringComparison]::Ordinal) -lt $chain.IndexOf($step,[StringComparison]::Ordinal)) 'Native chain lacks its original pre-risk prepared receipt.'
Assert-ChainWorkflow ($chain.Contains('mode: [complete, inner-watchdog, outer-runner]') -and
    $chain -cmatch '(?m)^    continue-on-error: \$\{\{ matrix.mode == ''outer-runner'' \}\}\r?$') 'Only the separate outer runner negative may tolerate job failure.'
Assert-ChainWorkflow (-not ($step -match '(?m)^        (if|continue-on-error):') -and $step.Contains('invoke-msi-feedback-chain.ps1')) 'Chain runner step lost ordinary operation gating.'
Assert-ChainWorkflow ($chain.Contains('-Experiment chain') -and $chain.Contains('actions: read') -and $chain.Contains('checks: read')) 'Independent native chain service verifier is missing.'
$invoke=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/scripts/invoke-msi-feedback-chain.ps1')
$tokens=$null;$errors=$null
$ast=[Management.Automation.Language.Parser]::ParseInput($invoke,[ref]$tokens,[ref]$errors)
Assert-ChainWorkflow ($errors.Count -eq 0) 'Actual chain entry does not parse.'
$commands=@($ast.FindAll({param($node) $node -is [Management.Automation.Language.CommandAst]},$true))
$names=@($commands | ForEach-Object {$_.GetCommandName()})
Assert-ChainWorkflow ($names -ccontains 'Assert-P4kxMsiStepAdmission' -and $names -ccontains 'Invoke-P4kxMsiDiagnosticWorker') 'Chain does not enter actual admission and parent.'
$forbidden=@('check-msi.ps1','Invoke-P4kxMsiexec','msiexec','New-Object','Set-ItemProperty','Remove-ItemProperty','reg.exe')
Assert-ChainWorkflow (@($names | Where-Object {$_ -cin $forbidden}).Count -eq 0) 'Diagnostic chain invokes an installer, COM or registry adapter.'
$firstAdmission=@($commands | Where-Object {$_.GetCommandName() -ceq 'Assert-P4kxMsiStepAdmission'}) | Select-Object -First 1
$parent=@($commands | Where-Object {$_.GetCommandName() -ceq 'Invoke-P4kxMsiDiagnosticWorker'}) | Select-Object -First 1
Assert-ChainWorkflow ($firstAdmission.Extent.StartOffset -lt $parent.Extent.StartOffset -and $invoke.Contains("finally{`$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC=`$originalDeadline}")) 'Fixture deadline shortening escaped admission or its restoration scope.'
$literal=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/workflows/msi-feedback-probe.yml')
$literalStep=Get-ChainWorkflowStep $literal 'Runner step fuse fixture'
Assert-ChainWorkflow ($literalStep -cmatch '(?m)^        timeout-minutes: 1\r?$' -and $literalStep.Contains('invoke-msi-feedback-probe.ps1')) 'Independent literal probe was replaced or silently reinterpreted.'
'PASS: actual pre-risk evidence and separated dynamic chain source contracts; no installer execution or original-hang reproduction.'
