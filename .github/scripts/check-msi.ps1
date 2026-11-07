param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Msi,
    [Parameter(Mandatory)][string]$NextMsi,
    [Parameter(Mandatory)][string]$NextVersion,
    [Parameter(Mandatory)][string]$UpgradeCode,
    [Parameter(Mandatory)][string]$OtherArchitectureMsi,
    [Parameter(Mandatory)][string]$FixtureDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$PreviousMsi
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'msi-installation.ps1')
. (Join-Path $PSScriptRoot 'binary-machine.ps1')
$row = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $row
if ($env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted') {
    throw 'Installer verification requires a disposable GitHub-hosted native Windows runner.'
}
$Msi = Resolve-P4kxMsiInput $Msi
$NextMsi = Resolve-P4kxMsiInput $NextMsi
$OtherArchitectureMsi = Resolve-P4kxMsiInput $OtherArchitectureMsi
if ($PreviousMsi) { $PreviousMsi = Resolve-P4kxMsiInput $PreviousMsi }
$current = Get-P4kxMsiMetadata $Msi
$next = Get-P4kxMsiMetadata $NextMsi
$opposite = @((Get-P4kxPlatformContract).targets | Where-Object { $_.os -eq 'windows' -and $_.arch -ne $row.arch })[0]
$other = Get-P4kxMsiMetadata $OtherArchitectureMsi
Assert-P4kxMsiMetadata $current $row $current.version $UpgradeCode
Assert-P4kxMsiMetadata $next $row $NextVersion $UpgradeCode
Assert-P4kxMsiMetadata $other $opposite $current.version $UpgradeCode
if (@(@($current.product, $next.product, $other.product) | Select-Object -Unique).Count -ne 3 -or
    @(@($current.package, $next.package, $other.package) | Select-Object -Unique).Count -ne 3) { throw 'MSI package/product identities collide.' }
foreach ($component in $current.components) {
    if ($component[0] -in @($other.components | ForEach-Object { $_[0] })) { throw 'CPU packages share a component GUID.' }
}
$folder = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Programs/P4KX'
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'P4KX.lnk'
$profileFolder = Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'P4KX'
$logs = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$originalPath = [Environment]::GetEnvironmentVariable('Path', 'User')
Assert-P4kxUninstalled $UpgradeCode $folder $shortcut $originalPath
if (Test-Path -LiteralPath $profileFolder) { throw 'Installer verification requires an absent P4KX profile.' }
New-Item -ItemType Directory -Path $profileFolder | Out-Null
$markers = @('settings.toml', 'archive.key')
$markerHashes = @{}
foreach ($marker in $markers) {
    'synthetic installer preservation marker' | Set-Content (Join-Path $profileFolder $marker) -Encoding utf8NoBOM
    $markerHashes[$marker] = (Get-FileHash (Join-Path $profileFolder $marker)).Hash
}

function Assert-ProfilePreserved {
    foreach ($marker in $markers) {
        if ((Get-FileHash (Join-Path $profileFolder $marker)).Hash -cne $markerHashes[$marker]) { throw "Installer changed the profile marker: $marker" }
    }
}

function Test-InstalledPayload([string]$Name) {
    & (Join-Path $PSScriptRoot 'check-package-runtime.ps1') -Target $Target -BinaryDirectory $folder -FixtureDirectory $FixtureDirectory -OutputDirectory (Join-Path $logs $Name) -Package msi
    Assert-ProfilePreserved
}

$verificationFailure = $null
try {
    if ($PreviousMsi) {
        if ($row.arch -cne 'amd64') { throw 'Historical AMD64 MSI is not an ARM64 upgrade predecessor.' }
        $previous = Get-P4kxMsiMetadata $PreviousMsi
        if ($previous.template -cne 'x64;1033' -or $previous.version -cne '0.2.0' -or
            $previous.upgrade -ine $current.upgrade) { throw 'Historical MSI CPU/version/upgrade identity differs.' }
        $oldKeys = @(Get-P4kxPredecessorRegistryKeys $previous)
        Invoke-P4kxMsiexec @('/i', $PreviousMsi) (Join-Path $logs 'previous-install.log')
        Invoke-P4kxMsiexec @('/i', $Msi) (Join-Path $logs 'rename-upgrade.log')
        $oldFolder = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Programs/P4K-Extractor'
        if (Test-Path -LiteralPath $oldFolder) { throw 'Historical install directory survived migration.' }
        if ([Environment]::GetEnvironmentVariable('Path', 'User') -match 'Programs\\P4K-Extractor') { throw 'Historical PATH entry survived migration.' }
        foreach ($key in $oldKeys) { if (Test-Path -LiteralPath "HKCU:\$key") { throw 'Historical installer registry identity survived migration.' } }
        Assert-P4kxInstalledProduct $current $folder $shortcut
        Test-InstalledPayload 'historical-upgrade'
        Invoke-P4kxMsiexec @('/x', $current.product) (Join-Path $logs 'historical-uninstall.log')
        Assert-P4kxUninstalled $UpgradeCode $folder $shortcut $originalPath
    }
    Invoke-P4kxMsiexec @('/i', $Msi) (Join-Path $logs 'install.log')
    Assert-P4kxInstalledProduct $current $folder $shortcut
    Test-InstalledPayload 'installed'
    $before = @{}
    foreach ($file in Get-ChildItem -LiteralPath $folder -File) { $before[$file.Name] = (Get-FileHash $file.FullName).Hash }
    $rollbackMsi = Join-Path $logs 'rollback-probe.msi'
    New-P4kxRollbackMsi $NextMsi $rollbackMsi
    Invoke-P4kxMsiexec @('/i', $rollbackMsi) (Join-Path $logs 'rollback.log') 1603
    $rollbackLog = Get-Content (Join-Path $logs 'rollback.log') -Raw
    if ($rollbackLog -notmatch 'Action ended .*InstallExecute\. Return value 1\.' -or
        $rollbackLog -notmatch 'Action ended .*P4kxRollbackProbe\. Return value 3\.' -or
        $rollbackLog -notmatch 'Rollback') { throw 'MSI failure did not demonstrate post-execution rollback.' }
    Assert-P4kxInstalledProduct $current $folder $shortcut
    foreach ($name in $before.Keys) {
        if ((Get-FileHash (Join-Path $folder $name)).Hash -cne $before[$name]) { throw "Rollback did not restore installed file: $name" }
    }
    Test-InstalledPayload 'rollback-restored'
    Invoke-P4kxMsiexec @('/i', $NextMsi) (Join-Path $logs 'upgrade.log')
    Assert-P4kxInstalledProduct $next $folder $shortcut
    Test-InstalledPayload 'upgraded'
    Invoke-P4kxMsiexec @('/i', $Msi) (Join-Path $logs 'downgrade-blocked.log') 1603
    Assert-P4kxInstalledProduct $next $folder $shortcut
    Assert-ProfilePreserved
    Invoke-P4kxMsiexec @('/x', $next.product) (Join-Path $logs 'uninstall.log')
    Assert-P4kxUninstalled $UpgradeCode $folder $shortcut $originalPath
    Assert-ProfilePreserved
    if ($row.arch -eq 'arm64') {
        Invoke-P4kxMsiexec @('/i', $OtherArchitectureMsi) (Join-Path $logs 'amd64-compatibility-install.log')
        Assert-P4kxInstalledProduct $other $folder $shortcut
        Invoke-P4kxMsiexec @('/i', $Msi) (Join-Path $logs 'same-version-native-replacement.log')
        Assert-P4kxInstalledProduct $current $folder $shortcut
        Test-InstalledPayload 'native-replacement'
        Invoke-P4kxMsiexec @('/i', $OtherArchitectureMsi) (Join-Path $logs 'same-version-amd64-replacement.log')
        Assert-P4kxInstalledProduct $other $folder $shortcut
        if ((Get-ImageMachine -Path (Join-Path $folder 'p4kx.exe') -Os windows) -ne $opposite.pe_machine) { throw 'Explicit AMD64 replacement payload differs.' }
        Invoke-P4kxMsiexec @('/x', $other.product) (Join-Path $logs 'replacement-uninstall.log')
    }
    else { Invoke-P4kxMsiexec @('/i', $OtherArchitectureMsi) (Join-Path $logs 'wrong-cpu-rejected.log') 1633 }
    Assert-P4kxUninstalled $UpgradeCode $folder $shortcut $originalPath
    Assert-ProfilePreserved
    [ordered]@{
        target=$Target; lifecycle='success'; rollback='success'; profile_preserved=$true
        cross_cpu=$(if ($row.arch -eq 'arm64') { 'explicit replacement both ways; AMD64 compatibility is not native evidence' } else { 'ARM64 package rejected' })
    } | ConvertTo-Json | Set-Content (Join-Path $logs 'installer.json') -Encoding utf8NoBOM
}
catch {
    $verificationFailure = $_
    throw
}
finally {
    try {
        foreach ($product in @(Get-P4kxInstalledProducts $UpgradeCode)) { Invoke-P4kxMsiexec @('/x', $product) (Join-Path $logs 'cleanup-uninstall.log') }
        foreach ($marker in $markers) {
            $path = Join-Path $profileFolder $marker
            if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path }
        }
        if ((Get-ChildItem -LiteralPath $profileFolder -Force).Count -eq 0) { Remove-Item -LiteralPath $profileFolder }
    }
    catch {
        if ($null -ne $verificationFailure) {
            throw [AggregateException]::new('Installer verification and cleanup both failed.', [Exception[]]@($verificationFailure.Exception, $_.Exception))
        }
        throw
    }
}
