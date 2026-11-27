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
$release=Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/workflows/release-windows-execute.yml')
$prepare=Get-ChainWorkflowStep $release 'Prepare installer inputs and the independent step allowance'
$installer=Get-ChainWorkflowStep $release 'Installer metadata, upgrade, downgrade, rollback, CPU replacement and preservation'
$collected=Get-ChainWorkflowStep $release 'Finish the original native MSI lifecycle'
$logs=@([regex]::Matches($release,'(?ms)^      - uses: actions/upload-artifact@[^\r\n]+\r?\n.*?(?=^      - |\z)') |
    Where-Object {$_.Value.Contains('name: windows-package-logs-')})
Assert-ChainWorkflow ($logs.Count -eq 1) 'Expected one aggregate native diagnostic upload.'
$diagnostics=$logs[0].Value
Assert-ChainWorkflow ($release.IndexOf($prepare,[StringComparison]::Ordinal) -lt $release.IndexOf($installer,[StringComparison]::Ordinal) -and
    $release.IndexOf($installer,[StringComparison]::Ordinal) -lt $release.IndexOf($collected,[StringComparison]::Ordinal) -and
    $release.IndexOf($collected,[StringComparison]::Ordinal) -lt $release.IndexOf($diagnostics,[StringComparison]::Ordinal)) 'Prepare and collect the original native lifecycle before diagnostic transport.'
Assert-ChainWorkflow ($diagnostics -cmatch '(?m)^        if: always\(\)\r?$' -and
    $diagnostics -cmatch '(?m)^        timeout-minutes: 5\r?$' -and
    $diagnostics.Contains('installer-check/**/*.json') -and
    $diagnostics.Contains('contracts/msi-feedback-parent/**/*.json') -and
    $diagnostics.Contains('contracts/msi-feedback-parent/**/*.jsonl') -and
    $diagnostics.Contains('contracts/msi-feedback-parent/**/*.log')) 'The bounded aggregate upload must retain prepared allowance and parent-control evidence.'
Assert-ChainWorkflow ($prepare.Contains('Write-P4kxMsiDiagnosticSources $parameters .github/scripts $feedback') -and
    $prepare.Contains('Write-P4kxMsiDiagnosticInputs $parameters $feedback') -and
    $prepare.Contains('$output = Join-Path $env:P4KX_PACKAGE_TASK ''installer-check''') -and
    $prepare.Contains('$feedback = Join-Path $output ''feedback''') -and
    $prepare.Contains("'msi-invocation.json'") -and $installer.Contains("'msi-invocation.json'") -and
    $installer.Contains('invoke-msi-observer.ps1 -Stage Start') -and
    (Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/scripts/msi-observer-parent.ps1')).Contains("'check-msi.ps1') @parameters") -and
    -not $diagnostics.Contains('msi-invocation.json')) 'Original input hashes must be recorded before the same local invocation is consumed, with its private manifest kept on the runner.'

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
'PASS: local native evidence, deferred transport and separate dynamic chain source contracts; no installer execution or original-hang reproduction.'
