$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-path-fixture.ps1')
. (Join-Path $PSScriptRoot 'msi-installation.ps1')
. (Join-Path $PSScriptRoot 'msi-raw-fixture.ps1')

function ConvertTo-P4kxRawPathFixture {
    param($State)
    if(-not $State.exists){return $null}
    $bytes=[Text.Encoding]::Unicode.GetBytes($State.raw_value+[char]0)
    return @{registry_type=$(if($State.registry_kind -ceq 'String'){1}else{2});bytes_hex=[Convert]::ToHexString($bytes).ToLowerInvariant()}
}

function Assert-P4kxRawMsiInstalled {
    param($Original,$Installed,$Metadata,[string]$Folder,[string]$Phase='committed')
    $owner=$Installed.core.owner
    if($Installed.core.status -cne 'valid' -or $Installed.core.phase -cne $Phase -or $null -eq $owner -or
        $owner.product -cne ([guid]$Metadata.product).ToString('N') -or $owner.package -cne ([guid]$Metadata.package).ToString('N')){throw 'Actual helper owner/WAL differs from the installed MSI identity.'}
    if($owner.claim -ceq 'borrowed'){Assert-P4kxRawMsiPathEqual $Original $Installed;return}
    if($owner.claim -cne 'inserted'){throw 'Unknown PATH claim.'}
    $entry=[Convert]::ToHexString([Text.Encoding]::Unicode.GetBytes($Folder.TrimEnd('\')+'\')).ToLowerInvariant()
    $expected=if($null -eq $Original.image.path){@{registry_type=1;bytes_hex=$entry+'0000'}}else{@{registry_type=$Original.image.path.registry_type;bytes_hex=$entry+'3b00'+$Original.image.path.bytes_hex}}
    if(-not (Test-P4kxRawMsiValueEqual $expected $Installed.image.path)){throw 'Installed PATH changed foreign bytes, registry kind or terminator spelling.'}
}

function Restore-P4kxMsiVerificationBaseline {
    param($Context,$Fixture,$Original)
    Assert-P4kxMsiMutationReady $Fixture
    $index=0
    foreach($product in @(Get-P4kxInstalledProducts $Context.upgrade)){
        Invoke-P4kxMsiexec @('/x',$product) (Join-Path $Context.logs "raw-cleanup-$index.log") -RawFixture $Fixture | Out-Null
        $index++
    }
    Assert-P4kxInstallationOwnershipAbsent $Context.upgrade $Context.folder $Context.shortcut
    Assert-P4kxLegacyOwnershipAbsent $Context
    $beforeReset=Get-P4kxRawMsiImage $Fixture
    $afterReset=Restore-P4kxRawMsiImage $Fixture $beforeReset $Original
    return @{before=$beforeReset;after=$afterReset;fixture_only=$true}
}

function Invoke-P4kxMsiRawPathCases {
    param($Context,$Fixture,$Original,[Collections.Generic.List[Exception]]$Failures)
    Assert-P4kxMsiMutationReady $Fixture
    $results=@()
    foreach($case in Get-P4kxPathFixtureCases){
        $before=$null;$installed=$null;$after=$null;$reset=$null;$failure=$null
        try{
            Assert-P4kxInstallationOwnershipAbsent $Context.upgrade $Context.folder $Context.shortcut
            $before=Set-P4kxRawMsiPath $Fixture (Get-P4kxRawMsiImage $Fixture) (ConvertTo-P4kxRawPathFixture $case.state)
            $install=Invoke-P4kxMsiexec @('/i',$Context.msi) (Join-Path $Context.logs ($case.name+'-install.log')) -RawFixture $Fixture
            Assert-P4kxInstalledProduct $Context.metadata $Context.folder $Context.shortcut
            $installed=$install.raw_after
            Assert-P4kxRawMsiInstalled $before $installed $Context.metadata $Context.folder
            if($installed.core.owner.claim -cne 'inserted'){throw 'Synthetic raw case unexpectedly borrowed an existing entry.'}
            $uninstall=Invoke-P4kxMsiexec @('/x',$Context.metadata.product) (Join-Path $Context.logs ($case.name+'-uninstall.log')) -RawFixture $Fixture
            Assert-P4kxInstallationOwnershipAbsent $Context.upgrade $Context.folder $Context.shortcut
            $after=$uninstall.raw_after
            Assert-P4kxRawMsiPathEqual $before $after
        }catch{$failure=$_.Exception;$Failures.Add($failure)}
        try{$reset=Restore-P4kxMsiVerificationBaseline $Context $Fixture $Original}catch{
            if($null -ne $failure){throw [AggregateException]::new('Raw PATH verification and ownership cleanup/reset failed.',[Exception[]]@($failure,$_.Exception))}
            throw
        }
        $results+=@{name=$case.name;outcome=$(if($null -eq $failure){'success'}else{'failed'});before=$before;installed=$installed;after=$after;fixture_reset=$reset;
            error=$(if($null -ne $failure){$failure.Message}else{$null})}
    }
    return ,$results
}

function Invoke-P4kxMsiPathHelper {
    param($Context,[string[]]$Arguments,[Parameter(Mandatory)]$Fixture)
    Assert-P4kxMsiMutationReady $Fixture
    if((Get-FileHash -LiteralPath $Context.helper -Algorithm SHA256).Hash.ToLowerInvariant() -cne $Context.helper_sha256){throw 'Executed helper changed after MSI Binary binding.'}
    $Context.helper_calls++
    try{$result=Invoke-P4kxPortalProcess $Context.helper $Arguments $Context.logs ('path-helper-'+$Context.helper_calls)}
    catch{$Fixture.interrupted=$true;throw}
    if($result.exit_code -ne 0){$errorValue=[InvalidOperationException]::new('Native PATH helper probe failed.');$errorValue.Data['result']=$result;throw $errorValue}
}

function Invoke-P4kxMsiForeignEditProbe {
    param($Context,$Fixture,$Original)
    Assert-P4kxMsiMutationReady $Fixture
    $binding=$null;$failure=$null;$result=$null
    try{
        Assert-P4kxInstallationOwnershipAbsent $Context.upgrade $Context.folder $Context.shortcut
        $value=@{registry_type=2;bytes_hex=[Convert]::ToHexString([Text.Encoding]::Unicode.GetBytes('C:\P4kxSynthetic\base'+[char]0)).ToLowerInvariant()}
        $before=Set-P4kxRawMsiPath $Fixture (Get-P4kxRawMsiImage $Fixture) $value
        Invoke-P4kxMsiPathHelper $Context @('reserve',$Context.metadata.product,$Context.metadata.package) $Fixture
        $reserved=Get-P4kxRawMsiImage $Fixture;$binding=$reserved.core.reserved_binding
        if($reserved.core.status -cne 'valid' -or $binding -cnotmatch '\Av1\|[0-9]+\|[0-9a-f]{32}\|[0-9a-f]{32}\|[0-9a-f]{64}\z'){throw 'Actual reservation binding is missing.'}
        if($reserved.core.reservation_phase -cne 'neutral'){throw 'Early reservation already contains a costing-dependent goal.'}
        Invoke-P4kxMsiPathHelper $Context @('seal','ensure',$Context.metadata.product,$Context.metadata.package,$binding,'',($Context.folder.TrimEnd('\')+'\.')) $Fixture
        $sealed=Get-P4kxRawMsiImage $Fixture
        if($sealed.core.reserved_binding -cne $binding -or $sealed.core.reservation_phase -cne 'sealed'){throw 'Seal changed binding or did not retain the intent.'}
        foreach($phase in @('prepare','apply')){Invoke-P4kxMsiPathHelper $Context @($phase,$Context.metadata.product,$Context.metadata.package,$binding) $Fixture}
        $applied=Get-P4kxRawMsiImage $Fixture
        Assert-P4kxRawMsiInstalled $before $applied $Context.metadata $Context.folder applied
        $prefix=[Convert]::ToHexString([Text.Encoding]::Unicode.GetBytes('C:\P4kxSynthetic\external;')).ToLowerInvariant()
        $foreign=Set-P4kxRawMsiPath $Fixture $applied @{registry_type=2;bytes_hex=$prefix+$applied.image.path.bytes_hex}
        Invoke-P4kxMsiPathHelper $Context @('rollback',$Context.metadata.product,$Context.metadata.package,$binding) $Fixture
        $after=Get-P4kxRawMsiImage $Fixture
        if($after.core.phase -cne 'rolledBack' -or $after.core.journal_binding -cne $binding -or $null -ne $after.core.owner -or
            -not (Test-P4kxRawMsiValueEqual $after.image.path @{registry_type=2;bytes_hex=$prefix+$before.image.path.bytes_hex})){throw 'Native rollback lost the foreign PATH edit or caller binding.'}
        $result=@{outcome='success';before=$before;applied=$applied;foreign=$foreign;after=$after}
    }catch{$failure=$_.Exception}
    try{
        if($null -ne $binding){Invoke-P4kxMsiPathHelper $Context @('rollback',$Context.metadata.product,$Context.metadata.package,$binding) $Fixture}
        $reset=Restore-P4kxMsiVerificationBaseline $Context $Fixture $Original
        if($null -ne $result){$result.fixture_reset=$reset}
    }catch{if($null -ne $failure){throw [AggregateException]::new('Native adapter probe and cleanup failed.',[Exception[]]@($failure,$_.Exception))};throw}
    if($null -ne $failure){throw $failure}
    return $result
}

function Assert-P4kxMsiReservedCleanup {
    param($Before,$After,$Metadata)
    Assert-P4kxRawMsiPathEqual $Before $After
    $prefix='v1|'+$Before.core.sequence+'|'+([guid]$Metadata.product).ToString('N')+'|'+([guid]$Metadata.package).ToString('N')+'|'
    if($Before.core.status -cne 'valid' -or $After.core.status -cne 'valid' -or
        $Before.core.sequence -cne $After.core.sequence -or $Before.core.phase -cne $After.core.phase -or
        $Before.core.journal_binding -cne $After.core.journal_binding -or $null -ne $After.core.reserved_binding -or
        $null -ne $After.core.reservation_phase -or $After.core.cancelled_binding -isnot [string] -or
        -not $After.core.cancelled_binding.StartsWith($prefix,[StringComparison]::Ordinal) -or
        $After.core.cancelled_binding.Substring($prefix.Length) -cnotmatch '\A[0-9a-f]{64}\z'){
        throw 'Noop or disabled-rollback cleanup did not retain the exact terminal state and cancellation fence.'
    }
    if($null -eq $Before.core.owner -or $null -eq $After.core.owner){
        if($null -ne $Before.core.owner -or $null -ne $After.core.owner){throw 'Reservation cleanup changed owner presence.'}
    }else{
        foreach($field in @('product','package','claim')){
            if($Before.core.owner.$field -cne $After.core.owner.$field){throw 'Reservation cleanup changed the terminal owner.'}
        }
    }
}

function Assert-P4kxMsiPathVerification {
    param($Report,$Platform,[string]$Revision,[string]$MsiHash)
    if($MsiHash -cnotmatch '\A[0-9a-f]{64}\z' -or $Revision -cnotmatch '\A[0-9a-f]{40}(?:[0-9a-f]{24})?\z' -or
        $Report.folder -isnot [string] -or [string]::IsNullOrEmpty($Report.folder) -or $Report.raw_root -isnot [string] -or [string]::IsNullOrEmpty($Report.raw_root) -or
        $Report.schema -ne 1 -or $Report.scope -cne 'native-synthetic' -or $Report.target -cne $Platform.target -or $Report.revision -cne $Revision -or
        $Report.msi_sha256 -cne $MsiHash -or $Report.helper_sha256 -cnotmatch '\A[0-9a-f]{64}\z' -or $Report.fixture_sha256 -cnotmatch '\A[0-9a-f]{64}\z' -or
        $Report.run -cnotmatch '\A[1-9][0-9]*\z' -or $Report.attempt -cnotmatch '\A[1-9][0-9]*\z' -or $Report.adapter.outcome -cne 'success'){
        throw 'MSI PATH source/target/probe evidence is missing or failed.'
    }
    $context=@{revision=$Revision;run=$Report.run;attempt=$Report.attempt;root=$Report.raw_root;arch=$(if($Platform.arch -ceq 'amd64'){'x86_64'}else{'aarch64'})}
    $expected=@(Get-P4kxPathFixtureCases)
    if((($Report.raw_cases.name | Sort-Object) -join ',') -cne (($expected.name | Sort-Object) -join ',')){throw 'The eight exact PATH cases did not execute exactly once.'}
    foreach($case in $Report.raw_cases){
        if($case.outcome -cne 'success' -or $null -ne $case.error -or $case.fixture_reset.fixture_only -isnot [bool] -or -not $case.fixture_reset.fixture_only){throw 'A raw PATH case failed or lacks independent cleanup/reset.'}
        foreach($snapshot in @($case.before,$case.installed,$case.after,$case.fixture_reset.before,$case.fixture_reset.after)){Assert-P4kxRawMsiEnvelope $context $snapshot}
        $definition=@($expected | Where-Object name -CEQ $case.name)[0]
        if(-not (Test-P4kxRawMsiValueEqual $case.before.image.path (ConvertTo-P4kxRawPathFixture $definition.state))){throw 'Observed raw seed differs from the declared original eight cases.'}
        Assert-P4kxRawMsiPathEqual $case.before $case.after
        Assert-P4kxRawMsiInstalled $case.before $case.installed $Report.msi_identity $Report.folder
        if($case.installed.core.owner.claim -cne 'inserted'){throw 'Synthetic PATH case lacks an actual insertion claim.'}
    }
    Assert-P4kxMsiAdapterEvidence $Report.adapter $Report $context
    $required=@{install=0;'feature-noop'=0;'rollback-disabled'=1603;repair=0;rollback=1603;upgrade=0;'downgrade-blocked'=1603;uninstall=0}
    if($Platform.arch -ceq 'arm64'){
        foreach($name in @('amd64-compatibility-install','same-version-native-replacement','same-version-amd64-replacement','replacement-uninstall')){$required[$name]=0}
    }else{$required['wrong-cpu-rejected']=1633}
    foreach($name in $required.Keys){
        $steps=@($Report.lifecycle_steps | Where-Object name -CEQ $name)
        if($steps.Count -ne 1 -or $steps[0].exit_code -ne $required[$name]){throw 'Required MSI lifecycle action or actual exit code is missing.'}
        Assert-P4kxRawMsiEnvelope $context $steps[0].before
        Assert-P4kxRawMsiEnvelope $context $steps[0].after
        if($name -cin @('feature-noop','rollback-disabled')){Assert-P4kxMsiReservedCleanup $steps[0].before $steps[0].after $Report.msi_identity}
        if($name -ceq 'downgrade-blocked'){
            foreach($slot in @('path','state','sequence','binding')){
                if(-not (Test-P4kxRawMsiValueEqual $steps[0].before.image[$slot] $steps[0].after.image[$slot])){throw 'Downgrade mutated the invocation store.'}
            }
        }
    }
    if($Platform.arch -ceq 'amd64' -and ($Report.legacy.calibration -cne 'success' -or $Report.legacy.upgrade -cne 'success')){throw 'Historical observed-cleanup calibration or migration failed.'}
}

function Assert-P4kxMsiAdapterEvidence {
    param($Adapter,$Report,$Context)
    foreach($snapshot in @($Adapter.before,$Adapter.applied,$Adapter.foreign,$Adapter.after,$Adapter.fixture_reset.before,$Adapter.fixture_reset.after)){Assert-P4kxRawMsiEnvelope $Context $snapshot}
    Assert-P4kxRawMsiInstalled $Adapter.before $Adapter.applied $Report.msi_identity $Report.folder applied
    $prefix=[Convert]::ToHexString([Text.Encoding]::Unicode.GetBytes('C:\P4kxSynthetic\external;')).ToLowerInvariant()
    if($Adapter.after.core.phase -cne 'rolledBack' -or $null -ne $Adapter.after.core.owner -or
        $Adapter.applied.core.journal_binding -cnotmatch '\Av1\|[0-9]+\|[0-9a-f]{32}\|[0-9a-f]{32}\|[0-9a-f]{64}\z' -or
        $Adapter.after.core.journal_binding -cne $Adapter.applied.core.journal_binding -or
        -not (Test-P4kxRawMsiValueEqual $Adapter.foreign.image.path @{registry_type=2;bytes_hex=$prefix+$Adapter.applied.image.path.bytes_hex}) -or
        -not (Test-P4kxRawMsiValueEqual $Adapter.after.image.path @{registry_type=2;bytes_hex=$prefix+$Adapter.before.image.path.bytes_hex}) -or
        $Adapter.fixture_reset.fixture_only -isnot [bool] -or -not $Adapter.fixture_reset.fixture_only){throw 'Native helper rollback did not preserve the external raw edit and captured binding.'}
}
