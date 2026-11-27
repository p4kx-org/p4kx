<#
.SYNOPSIS
Checks temporary Flatpak smoke launchers with native calls replaced by in-memory observations.
#>
param([string]$SourceDirectory=$PSScriptRoot)
$ErrorActionPreference = 'Stop'
$script:checks = 0
$script:calls = [Collections.Generic.List[object]]::new()
function Assert-Test([bool]$Value, [string]$Message) {
    if (-not $Value) { throw $Message }
    $script:checks++
}
function Assert-Arguments($Actual, $Expected, [string]$Message) {
    Assert-Test ($Actual.Count -eq $Expected.Count) $Message
    for ($index=0; $index -lt $Expected.Count; $index++) {
        Assert-Test ($Actual[$index] -ceq $Expected[$index]) $Message
    }
}
foreach ($binding in @(
    @{file='linux-package-installation.ps1';name='Get-P4kxFlatpakLaunchers'},
    @{file='package-launchers.ps1';name='Get-P4kxPackageArguments'}
)) {
    $tokens=$null; $errors=$null
    $ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $SourceDirectory $binding.file),[ref]$tokens,[ref]$errors)
    if ($errors.Count -ne 0) { throw 'Launcher source did not parse.' }
    $found=@($ast.FindAll({
        param($node)
        $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -ceq $binding.name
    },$true))
    if ($found.Count -ne 1) { throw 'Expected one actual launcher function.' }
    . ([ScriptBlock]::Create($found[0].Extent.Text))
}
function Get-P4kxLinuxPackaging {
    return @{application_id='io.github.p4kx_org.p4kx'}
}
function Invoke-P4kxLinuxTool {
    param([string]$Executable,[string[]]$Arguments)
    $script:calls.Add(@{executable=$Executable;arguments=$Arguments})
    Assert-Test ($Executable -ceq 'flatpak') 'Only the existing Flatpak driver query is permitted.'
    Assert-Arguments $Arguments @('run','--user',("--arch="+$script:arch),'--command=find','io.github.p4kx_org.p4kx','/usr/lib','-name','lvp_icd*.json','-type','f') 'Driver lookup changed or gained a filesystem grant.'
    return $script:driver
}
$fixture='/home/runner/task/fixture with spaces/雪'
$data='/home/runner/.var/app/io.github.p4kx_org.p4kx/data'
foreach ($architecture in @('x86_64','aarch64')) {
    $script:arch=$architecture
    $script:driver=[IO.Path]::GetFullPath((Join-Path $SourceDirectory "synthetic-drivers/lvp_icd.$architecture.json"))
    foreach ($scope in @('stock/flatpak','api5/smoke')) {
        $check="/home/runner/task/$scope"
        $launchers=Get-P4kxFlatpakLaunchers @{unix_arch=$architecture} $fixture $check $data
        Assert-Test ($launchers.Count -eq 3) 'Exactly three application launchers are required.'
        foreach ($name in @('p4kx','p4kx-gui','p4kx-mcp')) {
            $launcher=$launchers[$name]
            Assert-Test ($launcher.executable -ceq 'flatpak') 'The actual package launcher changed.'
            $expected=@('run','--user',("--arch="+$architecture),("--command="+$name),
                ("--env=P4KX_FOLDERS_ROOT="+$data+'/native-package-smoke'),'--env=P4KX_GAME_CHECK=off',
                ("--filesystem="+$fixture+':ro'),("--filesystem="+$check+':rw'))
            if ($name -ceq 'p4kx-gui') {
                $expected+=@('--env=P4KX_SMOKE_FRAMES=3',("--env=VK_DRIVER_FILES="+$script:driver),'--env=LIBGL_ALWAYS_SOFTWARE=1')
            }
            $expected+='io.github.p4kx_org.p4kx'
            Assert-Arguments $launcher.prefix $expected 'Scoped source/profile/output/Mesa argv changed.'
            $grants=@($launcher.prefix | Where-Object { $_.StartsWith('--filesystem=',[StringComparison]::Ordinal) })
            Assert-Arguments $grants @(("--filesystem="+$fixture+':ro'),("--filesystem="+$check+':rw')) 'Smoke has exactly its read-only fixture and current writable output.'
        }
        $output=$check+'/extract'
        $operation=@('extract',($fixture+'/native-smoke.p4k'),'--filter','*','--out',$output)
        $arguments=Get-P4kxPackageArguments $launchers.p4kx $operation
        Assert-Arguments $arguments (@($launchers.p4kx.prefix)+$operation) 'Actual CLI argv lost an argument boundary.'
        $index=[array]::IndexOf($arguments,'--out')
        Assert-Test ($index -ge 0 -and $arguments[$index+1] -ceq $output) 'The CLI output differs from the host-verified path.'
        $writable=@($arguments | Where-Object { $_ -ceq ("--filesystem="+$check+':rw') })
        Assert-Test ($writable.Count -eq 1 -and $output.StartsWith($check+'/',[StringComparison]::Ordinal)) 'Host output is outside the sole temporary writable grant.'
        if ($scope -ceq 'api5/smoke') {
            Assert-Test (@($arguments | Where-Object { $_ -like '--filesystem=*/stock/flatpak:rw' }).Count -eq 0) 'API5 reused the stale stock-session output grant.'
        }
    }
}
Assert-Test ($script:calls.Count -eq 4) 'One observed driver query is required per launcher build.'
"PASS: $script:checks Flatpak launcher/CLI argv checks across both CPUs and stock/API5 output scopes; four mocked driver queries, no native calls or file writes."
