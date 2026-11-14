param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Msi,
    [Parameter(Mandatory)][string]$NextMsi,
    [Parameter(Mandatory)][string]$NextVersion,
    [Parameter(Mandatory)][string]$UpgradeCode,
    [Parameter(Mandatory)][string]$OtherArchitectureMsi,
    [Parameter(Mandatory)][string]$FixtureDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$RegistryFixture,
    [Parameter(Mandatory)][string]$RegistryFixtureSha256,
    [Parameter(Mandatory)][string]$HelperExecutable,
    [string]$PreviousMsi,
    [switch]$LifecycleWorker,
    [string]$WorkerLocation,
    [string]$WorkerProcessDirectory
)

$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'msi-diagnostic-runner.ps1')
if (-not $LifecycleWorker) { Invoke-P4kxMsiDiagnosticWorker $PSCommandPath $PSBoundParameters; return }
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false,$true)
Set-Location -LiteralPath $WorkerLocation
[IO.Directory]::SetCurrentDirectory($WorkerProcessDirectory)
$diagnosticDirectory = Join-Path ([IO.Path]::GetFullPath($OutputDirectory)) 'diagnostics'
$diagnostics = New-P4kxMsiPhaseContext $diagnosticDirectory $Target
$workerPhase = Start-P4kxMsiPhase $diagnostics 'worker'
try {
$sourcePhase = Start-P4kxMsiPhase $diagnostics 'source-binding'
Write-P4kxMsiDiagnosticSources $PSBoundParameters $PSScriptRoot $diagnosticDirectory
Stop-P4kxMsiPhase $diagnostics $sourcePhase
$importPhase = Start-P4kxMsiPhase $diagnostics 'preamble/imports'
. (Join-Path $PSScriptRoot 'msi-path-verification.ps1')
. (Join-Path $PSScriptRoot 'msi-embedded-helper.ps1')
Stop-P4kxMsiPhase $diagnostics $importPhase
$guardPhase = Start-P4kxMsiPhase $diagnostics 'preamble/guards'
$row=Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost $row
if(-not $IsWindows -or $env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted'){throw 'MSI verification requires a disposable native Windows runner.'}
Stop-P4kxMsiPhase $diagnostics $guardPhase
$resolvePhase = Start-P4kxMsiPhase $diagnostics 'preamble/resolve-inputs'
$Msi=Resolve-P4kxMsiInput $Msi
$NextMsi=Resolve-P4kxMsiInput $NextMsi
$OtherArchitectureMsi=Resolve-P4kxMsiInput $OtherArchitectureMsi
if($PreviousMsi){$PreviousMsi=Resolve-P4kxMsiInput $PreviousMsi}
Stop-P4kxMsiPhase $diagnostics $resolvePhase
$metadataPhase = Start-P4kxMsiPhase $diagnostics 'preamble/current-metadata'
$current=Get-P4kxMsiMetadata $Msi
Stop-P4kxMsiPhase $diagnostics $metadataPhase
$metadataPhase = Start-P4kxMsiPhase $diagnostics 'preamble/next-metadata'
$next=Get-P4kxMsiMetadata $NextMsi
Stop-P4kxMsiPhase $diagnostics $metadataPhase
$opposite=@((Get-P4kxPlatformContract).targets | Where-Object { $_.os -eq 'windows' -and $_.arch -ne $row.arch })[0]
$metadataPhase = Start-P4kxMsiPhase $diagnostics 'preamble/opposite-metadata'
$other=Get-P4kxMsiMetadata $OtherArchitectureMsi
Stop-P4kxMsiPhase $diagnostics $metadataPhase
$transportPhase = Start-P4kxMsiPhase $diagnostics 'preamble/transport'
Assert-P4kxMsiMetadata $current $row $current.version $UpgradeCode
Assert-P4kxMsiMetadata $next $row $NextVersion $UpgradeCode
Assert-P4kxMsiMetadata $other $opposite $current.version $UpgradeCode
foreach($metadata in @($current,$next,$other)){Assert-P4kxMsiTransport $metadata}
if(@(@($current.product,$next.product,$other.product) | Select-Object -Unique).Count -ne 3 -or
    @(@($current.package,$next.package,$other.package) | Select-Object -Unique).Count -ne 3){throw 'MSI package/product identities collide.'}
foreach($component in $current.components){
    if($component[0] -in @($other.components | ForEach-Object { $_[0] })){throw 'CPU packages share a component GUID.'}
}
Stop-P4kxMsiPhase $diagnostics $transportPhase
$folder=Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Programs/P4KX'
$shortcut=Join-Path ([Environment]::GetFolderPath('Programs')) 'P4KX.lnk'
$profileFolder=Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'P4KX'
$logs=[IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$fixturePhase = Start-P4kxMsiPhase $diagnostics 'preamble/fixture-create'
$fixture=New-P4kxRawMsiFixture $row $RegistryFixture $RegistryFixtureSha256 (Join-Path $logs 'raw-registry')
Stop-P4kxMsiPhase $diagnostics $fixturePhase
$helperPhase = Start-P4kxMsiPhase $diagnostics 'preamble/helper-binding'
$helperHash=(Get-FileHash -LiteralPath $HelperExecutable -Algorithm SHA256).Hash.ToLowerInvariant()
if((Get-ImageMachine $HelperExecutable windows) -ne $row.pe_machine){throw 'Executed helper differs from the actual MSI Binary payload or native CPU.'}
$embeddedPhase = Start-P4kxMsiPhase $diagnostics 'preamble/current-embedded-helper'
$currentEmbedded = Get-P4kxMsiEmbeddedHelperDigest $Msi
Stop-P4kxMsiPhase $diagnostics $embeddedPhase
if($currentEmbedded.sha256 -cne $helperHash){throw 'Executed helper differs from the actual MSI Binary payload or native CPU.'}
$embeddedPhase = Start-P4kxMsiPhase $diagnostics 'preamble/next-embedded-helper'
$nextEmbedded = Get-P4kxMsiEmbeddedHelperDigest $NextMsi
Stop-P4kxMsiPhase $diagnostics $embeddedPhase
if($nextEmbedded.sha256 -cne $helperHash){throw 'Executed helper differs from the actual MSI Binary payload or native CPU.'}
Stop-P4kxMsiPhase $diagnostics $helperPhase
$bindingPhase = Start-P4kxMsiPhase $diagnostics 'input-binding'
Write-P4kxMsiDiagnosticInputs $PSBoundParameters $diagnosticDirectory
@{current=@{version=$current.version;template=$current.template}; next=@{version=$next.version;template=$next.template}; opposite=@{version=$other.version;template=$other.template}} |
    ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $diagnosticDirectory 'metadata-identities.json') -Encoding utf8NoBOM
Stop-P4kxMsiPhase $diagnostics $bindingPhase
$context=@{msi=$Msi;metadata=$current;upgrade=$UpgradeCode;folder=$folder;shortcut=$shortcut;logs=$logs;helper=[IO.Path]::GetFullPath($HelperExecutable);helper_sha256=$helperHash;helper_calls=0;legacy_keys=@()}
$capturePhase = Start-P4kxMsiPhase $diagnostics 'preamble/original-path'
$originalPath=Get-P4kxUserPathState
Stop-P4kxMsiPhase $diagnostics $capturePhase
$capturePhase = Start-P4kxMsiPhase $diagnostics 'preamble/original-raw'
$originalRaw=Get-P4kxRawMsiImage $fixture
Stop-P4kxMsiPhase $diagnostics $capturePhase
$preconditionPhase = Start-P4kxMsiPhase $diagnostics 'preamble/preconditions'
Assert-P4kxUninstalled $UpgradeCode $folder $shortcut $originalPath
if(Test-Path -LiteralPath $profileFolder){throw 'Installer verification requires an absent P4KX profile.'}
New-Item -ItemType Directory -Path $profileFolder | Out-Null
$markers=@('settings.toml','archive.key');$markerHashes=@{}
foreach($marker in $markers){
    'synthetic installer preservation marker' | Set-Content (Join-Path $profileFolder $marker) -Encoding utf8NoBOM
    $markerHashes[$marker]=(Get-FileHash (Join-Path $profileFolder $marker)).Hash
}
$steps=[Collections.Generic.List[object]]::new()
$failures=[Collections.Generic.List[Exception]]::new()
$legacy=$null;$rawCases=@();$adapter=$null;$lifecycle='failed';$primaryFailure=$null;$finalFailure=$null

function Assert-ProfilePreserved {
    foreach($marker in $markers){if((Get-FileHash (Join-Path $profileFolder $marker)).Hash -cne $markerHashes[$marker]){throw 'Installer changed a synthetic profile marker.'}}
}
function Test-InstalledPayload([string]$Name) {
    $payloadPhase=Start-P4kxMsiPhase $diagnostics ('runtime/'+$Name)
    try {
    & (Join-Path $PSScriptRoot 'check-package-runtime.ps1') -Target $Target -BinaryDirectory $folder -FixtureDirectory $FixtureDirectory -OutputDirectory (Join-Path $logs $Name) -Package msi
    Assert-ProfilePreserved
    Stop-P4kxMsiPhase $diagnostics $payloadPhase
    } catch { Stop-P4kxMsiPhase $diagnostics $payloadPhase -Failure $_.Exception; throw }
}
function Invoke-LifecycleMsi([string[]]$Arguments,[string]$Name,[int]$Code=0) {
    $lifecyclePhase=Start-P4kxMsiPhase $diagnostics ('lifecycle/'+$Name)
    try {
    $result=Invoke-P4kxMsiexec $Arguments (Join-Path $logs ($Name+'.log')) $Code -RawFixture $fixture
    $steps.Add(@{name=$Name;exit_code=$result.exit_code;before=$result.raw_before;after=$result.raw_after})
    Stop-P4kxMsiPhase $diagnostics $lifecyclePhase
    return $result
    } catch { Stop-P4kxMsiPhase $diagnostics $lifecyclePhase -Failure $_.Exception; throw }
}
function Assert-InstalledRaw($Metadata,[string]$Phase='committed') {
    Assert-P4kxInstalledProduct $Metadata $folder $shortcut
    Assert-P4kxRawMsiInstalled $originalRaw (Get-P4kxRawMsiImage $fixture) $Metadata $folder $Phase
}

try{
    Stop-P4kxMsiPhase $diagnostics $preconditionPhase
    if($PreviousMsi){
        try{
            if($row.arch -cne 'amd64'){throw 'Historical AMD64 MSI is not an ARM64 predecessor.'}
            $previousPhase=Start-P4kxMsiPhase $diagnostics 'preamble/historical-metadata'
            $previous=Get-P4kxMsiMetadata $PreviousMsi
            Stop-P4kxMsiPhase $diagnostics $previousPhase
            if($previous.template -cne 'x64;1033' -or $previous.version -cne '0.2.0' -or $previous.upgrade -ine $current.upgrade){throw 'Historical MSI identity differs.'}
            $oldKeys=@(Get-P4kxPredecessorRegistryKeys $previous);$context.legacy_keys=$oldKeys
            Invoke-LifecycleMsi @('/i',$PreviousMsi) 'legacy-calibration-install' | Out-Null
            $calibration=Invoke-LifecycleMsi @('/x',$previous.product) 'legacy-calibration-uninstall'
            Assert-P4kxInstallationOwnershipAbsent $UpgradeCode $folder $shortcut
            $rest=$calibration.raw_after
            $legacy=@{calibration='success';before=$originalRaw;observed_cleanup_rest=$rest;prior_mutation=(-not (Test-P4kxRawMsiValueEqual $originalRaw.image.path $rest.image.path))}
            $restorePhase=Start-P4kxMsiPhase $diagnostics 'restore/baseline' -RequiredCleanup
            Restore-P4kxMsiVerificationBaseline $context $fixture $originalRaw | Out-Null
            Stop-P4kxMsiPhase $diagnostics $restorePhase -RequiredCleanup
            Invoke-LifecycleMsi @('/i',$PreviousMsi) 'previous-install' | Out-Null
            Invoke-LifecycleMsi @('/i',$Msi) 'rename-upgrade' | Out-Null
            foreach($key in $oldKeys){if(Test-Path -LiteralPath "HKCU:\$key"){throw 'Historical registry identity survived migration.'}}
            $oldFolder=Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Programs/P4K-Extractor'
            if((Test-Path -LiteralPath $oldFolder) -or [Environment]::GetEnvironmentVariable('Path','User') -match 'Programs\\P4K-Extractor'){throw 'Historical installation survived migration.'}
            Assert-P4kxInstalledProduct $current $folder $shortcut
            Test-InstalledPayload 'historical-upgrade'
            $removed=Invoke-LifecycleMsi @('/x',$current.product) 'historical-uninstall'
            Assert-P4kxInstallationOwnershipAbsent $UpgradeCode $folder $shortcut
            Assert-P4kxRawMsiPathEqual $rest $removed.raw_after
            $legacy.upgrade='success';$legacy.after=$removed.raw_after
        }catch{$failures.Add($_.Exception);$legacy=@{outcome='failed';error=$_.Exception.Message}}
        $restorePhase=Start-P4kxMsiPhase $diagnostics 'restore/baseline' -RequiredCleanup
        Restore-P4kxMsiVerificationBaseline $context $fixture $originalRaw | Out-Null
        Stop-P4kxMsiPhase $diagnostics $restorePhase -RequiredCleanup
    }
    try{
        Invoke-LifecycleMsi @('/i',$Msi) 'install' | Out-Null
        Assert-InstalledRaw $current
        Test-InstalledPayload 'installed'
        $installed=Get-P4kxRawMsiImage $fixture
        $noop=Invoke-LifecycleMsi @('/i',$Msi) 'feature-noop'
        Assert-P4kxMsiReservedCleanup $noop.raw_before $noop.raw_after $current
        $noopLog=Get-Content (Join-Path $logs 'feature-noop.log') -Raw
        if($noopLog -notmatch 'Component: Application;[^\r\n]*Action: Null' -or
            $noopLog -notmatch 'Action ended [^\r\n]*P4kxPathCancelNoop\. Return value 1\.' -or
            $noopLog -match 'Action start [^\r\n]*P4kxPath(?:SealEnsure|SealRemove|Prepare)\.') {throw 'Maintenance did not execute the genuine no-component-action cancellation path.'}
        $disabled=Invoke-LifecycleMsi @('/i',$Msi,'REINSTALL=ALL','REINSTALLMODE=a','DISABLEROLLBACK=1') 'rollback-disabled' 1603
        Assert-P4kxMsiReservedCleanup $disabled.raw_before $disabled.raw_after $current
        $disabledLog=Get-Content (Join-Path $logs 'rollback-disabled.log') -Raw
        if($disabledLog -notmatch 'Action ended [^\r\n]*P4kxRequireRollback\. Return value 3\.' -or
            $disabledLog -match 'Action start [^\r\n]*P4kxPath(?:SealEnsure|SealRemove|Prepare)\.') {throw 'Rollback-disabled operation was not refused before sealing and prepare.'}
        Assert-InstalledRaw $current
        Invoke-LifecycleMsi @('/fa',$current.product) 'repair' | Out-Null
        Assert-InstalledRaw $current
        Assert-P4kxRawMsiPathEqual $installed (Get-P4kxRawMsiImage $fixture)
        Test-InstalledPayload 'repaired'
        $before=@{}
        foreach($file in Get-ChildItem -LiteralPath $folder -File){$before[$file.Name]=(Get-FileHash $file.FullName).Hash}
        $rollbackMsi=Join-Path $logs 'rollback-probe.msi'
        New-P4kxRollbackMsi $NextMsi $rollbackMsi
        Invoke-LifecycleMsi @('/i',$rollbackMsi) 'rollback' 1603 | Out-Null
        $rollbackLog=Get-Content (Join-Path $logs 'rollback.log') -Raw
        if($rollbackLog -notmatch 'Action ended .*InstallExecute\. Return value 1\.' -or $rollbackLog -notmatch 'Action ended .*P4kxRollbackProbe\. Return value 3\.' -or $rollbackLog -notmatch 'Rollback'){throw 'Failure did not demonstrate post-execute MSI rollback.'}
        Assert-InstalledRaw $current rolledBack
        Assert-P4kxRawMsiPathEqual $installed (Get-P4kxRawMsiImage $fixture)
        foreach($name in $before.Keys){if((Get-FileHash (Join-Path $folder $name)).Hash -cne $before[$name]){throw 'Rollback changed installed payload bytes.'}}
        Test-InstalledPayload 'rollback-restored'
        Invoke-LifecycleMsi @('/i',$NextMsi) 'upgrade' | Out-Null
        Assert-InstalledRaw $next
        Test-InstalledPayload 'upgraded'
        $blocked=Invoke-LifecycleMsi @('/i',$Msi) 'downgrade-blocked' 1603
        Assert-InstalledRaw $next
        Assert-P4kxRawMsiPathEqual $blocked.raw_before $blocked.raw_after
        Invoke-LifecycleMsi @('/x',$next.product) 'uninstall' | Out-Null
        Assert-P4kxUninstalled $UpgradeCode $folder $shortcut $originalPath
        Assert-P4kxRawMsiPathEqual $originalRaw (Get-P4kxRawMsiImage $fixture)
        if($row.arch -ceq 'arm64'){
            Invoke-LifecycleMsi @('/i',$OtherArchitectureMsi) 'amd64-compatibility-install' | Out-Null
            Assert-InstalledRaw $other
            Invoke-LifecycleMsi @('/i',$Msi) 'same-version-native-replacement' | Out-Null
            Assert-InstalledRaw $current
            Test-InstalledPayload 'native-replacement'
            Invoke-LifecycleMsi @('/i',$OtherArchitectureMsi) 'same-version-amd64-replacement' | Out-Null
            Assert-InstalledRaw $other
            if((Get-ImageMachine (Join-Path $folder 'p4kx.exe') windows) -ne $opposite.pe_machine){throw 'Explicit AMD64 replacement payload differs.'}
            Invoke-LifecycleMsi @('/x',$other.product) 'replacement-uninstall' | Out-Null
        }else{
            $wrong=Invoke-LifecycleMsi @('/i',$OtherArchitectureMsi) 'wrong-cpu-rejected' 1633
            Assert-P4kxRawMsiPathEqual $wrong.raw_before $wrong.raw_after
        }
        Assert-P4kxUninstalled $UpgradeCode $folder $shortcut $originalPath
        Assert-P4kxRawMsiPathEqual $originalRaw (Get-P4kxRawMsiImage $fixture)
        Assert-ProfilePreserved
        $lifecycle='success'
    }catch{$failures.Add($_.Exception)}
    $restorePhase=Start-P4kxMsiPhase $diagnostics 'restore/baseline' -RequiredCleanup
    $lifecycleReset=Restore-P4kxMsiVerificationBaseline $context $fixture $originalRaw
    Stop-P4kxMsiPhase $diagnostics $restorePhase -RequiredCleanup
    $rawPhase=Start-P4kxMsiPhase $diagnostics 'raw-path-cases'
    $rawCases=Invoke-P4kxMsiRawPathCases $context $fixture $originalRaw $failures
    Stop-P4kxMsiPhase $diagnostics $rawPhase
    $foreignPhase=Start-P4kxMsiPhase $diagnostics 'foreign-edit-probe'
    try{$adapter=Invoke-P4kxMsiForeignEditProbe $context $fixture $originalRaw;Stop-P4kxMsiPhase $diagnostics $foreignPhase}catch{Stop-P4kxMsiPhase $diagnostics $foreignPhase -Failure $_.Exception;$failures.Add($_.Exception);$adapter=@{outcome='failed';error=$_.Exception.Message}}
    $pathContract=@{schema=1;scope='native-synthetic';target=$Target;revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;
        msi_sha256=(Get-FileHash $Msi -Algorithm SHA256).Hash.ToLowerInvariant();helper_sha256=$helperHash;fixture_sha256=$RegistryFixtureSha256;msi_identity=@{product=$current.product;package=$current.package};
        folder=$folder;raw_root=$fixture.root;raw_cases=$rawCases;adapter=$adapter;lifecycle_reset=$lifecycleReset;lifecycle_steps=@($steps);legacy=$legacy}
    if($failures.Count -eq 0){Assert-P4kxMsiPathVerification $pathContract $row $env:GITHUB_SHA $pathContract.msi_sha256}
    @{target=$Target;lifecycle=$lifecycle;repair=$lifecycle;rollback=$lifecycle;profile_preserved=($failures.Count -eq 0);path_contract=$pathContract;
        cross_cpu=$(if($row.arch -ceq 'arm64'){'explicit replacement both ways; AMD64 compatibility is not native evidence'}else{'ARM64 package rejected'})} |
        ConvertTo-Json -Depth 12 | Set-Content (Join-Path $logs 'installer.json') -Encoding utf8NoBOM
    if($failures.Count -ne 0){$finalFailure=[AggregateException]::new('MSI lifecycle or exact raw PATH verification failed.',$failures);throw $finalFailure}
}catch{
    $primaryFailure=$_.Exception
    if($failures.Count -ne 0 -and -not [object]::ReferenceEquals($primaryFailure,$finalFailure)){
        $all=[Collections.Generic.List[Exception]]::new()
        foreach($failure in $failures){if(-not [object]::ReferenceEquals($failure,$primaryFailure)){$all.Add($failure)}}
        $all.Add($primaryFailure)
        $primaryFailure=[AggregateException]::new('MSI verification stopped after an earlier failed scenario.',$all)
    }
    throw $primaryFailure
}finally{
    $cleanupPhase=Start-P4kxMsiPhase $diagnostics 'final-cleanup' -RequiredCleanup
    $cleanupFailure=$null
    try{
    try{
        $restorePhase=Start-P4kxMsiPhase $diagnostics 'restore/baseline' -RequiredCleanup
        Restore-P4kxMsiVerificationBaseline $context $fixture $originalRaw | Out-Null
        Stop-P4kxMsiPhase $diagnostics $restorePhase -RequiredCleanup
        foreach($marker in $markers){$path=Join-Path $profileFolder $marker;if(Test-Path -LiteralPath $path){Remove-Item -LiteralPath $path}}
        if((Get-ChildItem -LiteralPath $profileFolder -Force).Count -eq 0){Remove-Item -LiteralPath $profileFolder}
        Write-P4kxPathTransition $originalPath (Get-P4kxUserPathState) (Join-Path $logs 'final-path.json')
    }catch{$cleanupFailure=$_.Exception;if($null -ne $primaryFailure){throw [AggregateException]::new('MSI verification and cleanup failed.',[Exception[]]@($primaryFailure,$_.Exception))};throw}
    }finally{ Stop-P4kxMsiPhase $diagnostics $cleanupPhase -Failure $cleanupFailure -RequiredCleanup }
}
$diagnosticFailure=Get-P4kxMsiDiagnosticFailure $diagnostics
if ($null -ne $diagnosticFailure) { throw $diagnosticFailure }
Stop-P4kxMsiPhase $diagnostics $workerPhase
} catch {
    $originalWorkerFailure=$_.Exception
    $workerFailure=Get-P4kxMsiDiagnosticFailure $diagnostics $originalWorkerFailure
    Stop-P4kxMsiPhase $diagnostics $workerPhase -Failure $workerFailure
    if ([object]::ReferenceEquals($workerFailure,$originalWorkerFailure)) { throw }
    throw $workerFailure
}
