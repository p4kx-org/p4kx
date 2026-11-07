$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'binary-machine.ps1')
. (Join-Path $PSScriptRoot 'portal-process.ps1')

<#
.SYNOPSIS
Refuses mutations after a native invocation without confirmed completion.
#>
function Assert-P4kxMsiMutationReady {
    param($Context)
    if($Context.interrupted){throw 'A native invocation has no proven completion; further fixture mutations are disabled.'}
}

function New-P4kxRawMsiFixture {
    param($Platform,[string]$Executable,[string]$ExpectedHash,[string]$Directory)
    Assert-P4kxNativeHost $Platform
    if (-not $IsWindows -or $env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or
        [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString().ToUpperInvariant() -cne $Platform.runner_arch.ToUpperInvariant()) {throw 'Raw MSI verification requires a disposable native Windows runner.'}
    if($ExpectedHash -cnotmatch '\A[0-9a-f]{64}\z' -or (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant() -cne $ExpectedHash -or
        (Get-ImageMachine $Executable windows) -ne $Platform.pe_machine) {throw 'Registry fixture executable source hash or CPU differs.'}
    $root=[IO.Path]::GetFullPath($Directory);$temporary=[IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\')
    if(-not $root.StartsWith($temporary+'\',[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $root)) {throw 'Raw fixture root must be a fresh child of RUNNER_TEMP.'}
    New-Item -ItemType Directory -Path $root | Out-Null
    return @{executable=[IO.Path]::GetFullPath($Executable);sha256=$ExpectedHash;root=$root;counter=0;interrupted=$false;revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;
        arch=$(if($Platform.arch -ceq 'amd64'){'x86_64'}else{'aarch64'});target=$Platform.target}
}

function New-P4kxRawMsiName {
    param($Context,[string]$Operation)
    if($Operation -cnotmatch '\A[a-z-]+\z' -or $Context.counter -ge 10000) {throw 'Raw fixture observation limit exceeded.'}
    $Context.counter++
    return ('raw-{0:d4}-{1}.json' -f $Context.counter,$Operation)
}

function Write-P4kxRawMsiJson {
    param([string]$Path,$Value)
    $bytes=[Text.UTF8Encoding]::new($false,$true).GetBytes(($Value | ConvertTo-Json -Depth 12 -Compress)+"`n")
    $stream=[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
    try{$stream.Write($bytes);$stream.Flush($true)}finally{$stream.Dispose()}
}

function Invoke-P4kxRawMsiFixture {
    param($Context,[string]$Command,[string[]]$Inputs=@())
    if($Command -cne 'capture'){Assert-P4kxMsiMutationReady $Context}
    if((Get-FileHash -LiteralPath $Context.executable -Algorithm SHA256).Hash.ToLowerInvariant() -cne $Context.sha256) {throw 'Registry fixture changed after source validation.'}
    $name=New-P4kxRawMsiName $Context $Command
    $environment=@{P4KX_NATIVE_REGISTRY_FIXTURE="p4kx-native-registry-fixture:$($Context.run):$($Context.attempt)"}
    try{$result=Invoke-P4kxPortalProcess $Context.executable (@($Command,$Context.root)+$Inputs+@($name)) $Context.root ([IO.Path]::GetFileNameWithoutExtension($name)) -Environment $environment}
    catch{$Context.interrupted=$true;throw}
    if($result.exit_code -ne 0) {$failure=[InvalidOperationException]::new('Raw Registry fixture failed.');$failure.Data['result']=$result;throw $failure}
    $path=Join-Path $Context.root $name
    if((Get-Item -LiteralPath $path).Length -gt 40MB) {throw 'Raw Registry observation exceeds its fixture bound.'}
    $value=Get-Content -LiteralPath $path -Raw | ConvertFrom-Json -AsHashtable -Depth 12
    Assert-P4kxRawMsiEnvelope $Context $value
    return $value
}

function Assert-P4kxRawMsiEnvelope {
    param($Context,$Value)
    $root=$Value.root -replace '\A\\\\\?\\UNC\\','\\' -replace '\A\\\\\?\\',''
    if($Value.schema -ne 1 -or $Value.role -cne 'disposable-ci-registry-fixture' -or $Value.revision -cne $Context.revision -or
        $Value.run -cne $Context.run -or $Value.attempt -cne $Context.attempt -or $Value.process_arch -cne $Context.arch -or
        -not [string]::Equals($root,$Context.root,[StringComparison]::OrdinalIgnoreCase) -or $Value.core.status -cnotin @('empty','valid','rejected')) {throw 'Registry observation source, run, CPU or root differs.'}
    $path=$Value.image.path
    if($null -ne $path -and ($path.registry_type -notin @(1,2) -or $path.bytes_hex -isnot [string] -or $path.bytes_hex.Length -gt 2MB -or $path.bytes_hex -cnotmatch '\A(?:[0-9a-f]{2})*\z')) {throw 'PATH observation is not exact raw string bytes.'}
}

function Get-P4kxRawMsiImage {
    param($Context)
    return Invoke-P4kxRawMsiFixture $Context capture
}

function Test-P4kxRawMsiValueEqual {
    param($Left,$Right)
    if($null -eq $Left -or $null -eq $Right) {return $null -eq $Left -and $null -eq $Right}
    return $Left.registry_type -eq $Right.registry_type -and $Left.bytes_hex -ceq $Right.bytes_hex
}

function Assert-P4kxRawMsiPathEqual {
    param($Before,$After)
    if(-not (Test-P4kxRawMsiValueEqual $Before.image.path $After.image.path)) {throw 'MSI changed original PATH existence, kind or bytes.'}
}

function Set-P4kxRawMsiPath {
    param($Context,$Expected,$Path)
    Assert-P4kxMsiMutationReady $Context
    $expectedName=New-P4kxRawMsiName $Context expected;$seedName=New-P4kxRawMsiName $Context seed
    Write-P4kxRawMsiJson (Join-Path $Context.root $expectedName) $Expected
    Write-P4kxRawMsiJson (Join-Path $Context.root $seedName) @{schema=1;path=$Path}
    $after=Invoke-P4kxRawMsiFixture $Context seed-path @($expectedName,$seedName)
    if(-not (Test-P4kxRawMsiValueEqual $after.image.path $Path)) {throw 'Native raw fixture seed differs from its declared bytes.'}
    return $after
}

function Restore-P4kxRawMsiImage {
    param($Context,$Expected,$Original)
    Assert-P4kxMsiMutationReady $Context
    if($Expected.core.status -ceq 'rejected' -or $null -ne $Expected.core.owner -or $null -ne $Expected.core.reserved_binding -or $Expected.core.phase -cin @('prepared','applied')) {throw 'Ownership must be resolved before fixture-only restoration.'}
    $expectedName=New-P4kxRawMsiName $Context expected;$originalName=New-P4kxRawMsiName $Context original
    Write-P4kxRawMsiJson (Join-Path $Context.root $expectedName) $Expected
    Write-P4kxRawMsiJson (Join-Path $Context.root $originalName) $Original
    $after=Invoke-P4kxRawMsiFixture $Context restore @($expectedName,$originalName)
    foreach($slot in @('path','state','sequence','binding')) {if(-not (Test-P4kxRawMsiValueEqual $after.image[$slot] $Original.image[$slot])) {throw 'Fixture reset did not restore its exact original raw image.'}}
    foreach($key in @('environment','installer','ownership','invocation')) {if($after.image.keys[$key] -cne $Original.image.keys[$key]) {throw 'Fixture reset changed original key existence.'}}
    return $after
}
