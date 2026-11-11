$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-consumer.ps1')
. (Join-Path $PSScriptRoot 'linux-package-lifecycle-evidence.ps1')
. (Join-Path $PSScriptRoot 'package-process.ps1')

function Assert-P4kxLifecycleDeployment {
    param($Producer, $Platform, [string]$Binaries, [string]$ShareDirectory)
    $images = @(Get-P4kxLinuxApplicationEvidence $Binaries $Platform)
    Assert-P4kxLinuxPackageExecution @{
        target=$Platform.target; package=$Producer.package; binaries=$images
    } $Producer $Platform
    $identity = (Get-P4kxLinuxPackaging).application_id
    $appstream = [xml](Get-Content -LiteralPath (Join-Path $ShareDirectory "metainfo/$identity.metainfo.xml") -Raw)
    $release = @($appstream.SelectNodes('/component/releases/release'))
    if ($appstream.component.id -cne $identity -or $release.Count -ne 1 -or
        $release[0].GetAttribute('version') -cne $Producer.version) {
        throw 'Installed AppStream identity or release version differs from the package producer.'
    }
}

function Get-P4kxLifecycleDebState {
    param($Producer, $Platform)
    $installed = Get-P4kxInstalledDeb $Platform
    if ($installed -cne (Get-P4kxDebianVersion $Producer.version)) {
        throw 'Observed Debian installed version differs from the package producer.'
    }
    $null = Get-P4kxDebLaunchers
    Assert-P4kxLifecycleDeployment $Producer $Platform '/usr/lib/p4kx' '/usr/share'
    return @{identity="p4kx:$($Platform.deb_arch)"; installed=$installed}
}

function Get-P4kxLifecycleFlatpakState {
    param($Producer, $Platform)
    $live = Get-P4kxLiveFlatpakIdentity $Platform $Producer
    $id = (Get-P4kxLinuxPackaging).application_id
    $location = (Invoke-P4kxLinuxTool flatpak @('info', '--user', "--arch=$($Platform.unix_arch)", '--show-location', $id) | Out-String).Trim()
    Assert-P4kxLifecycleDeployment $Producer $Platform (Join-Path $location 'files/bin') (Join-Path $location 'files/share')
    $folders = @(Invoke-P4kxLinuxTool flatpak @('run', '--user', "--arch=$($Platform.unix_arch)", '--command=printenv', $id, 'XDG_CONFIG_HOME', 'XDG_DATA_HOME'))
    $profile = Join-Path $env:HOME ".var/app/$id"
    if ($folders.Count -ne 2 -or $folders[0] -cne (Join-Path $profile 'config') -or $folders[1] -cne (Join-Path $profile 'data')) {
        throw 'Flatpak lifecycle profile differs from the installed application scope.'
    }
    return @{identity=$live.reference; installed=$live.app_commit; profile=$profile}
}

function Test-P4kxLifecyclePackageInstalled {
    param([string]$Format, $Platform)
    if ($Format -ceq 'deb') { return $null -ne (Get-P4kxInstalledDeb $Platform) }
    $applications = @(Invoke-P4kxLinuxTool flatpak @('list', '--user', '--app', '--columns=application'))
    return (Get-P4kxLinuxPackaging).application_id -cin $applications
}

function Invoke-P4kxDebLifecycleReplacement {
    param($Before, $After, $Platform, [string]$StockDirectory, [string]$Directory, [hashtable]$State)
    $null = Install-P4kxDeb $Before.asset $Before.proof.version $Platform
    $old = Get-P4kxLifecycleDebState $Before.proof $Platform
    Invoke-P4kxLinuxTool dpkg @('--compare-versions', $old.installed, 'lt', (Get-P4kxDebianVersion $After.proof.version)) | Out-Null
    if ($env:XDG_CONFIG_HOME -cne (Join-Path $StockDirectory 'config') -or
        $env:XDG_DATA_HOME -cne (Join-Path $StockDirectory 'data')) {
        throw 'Debian lifecycle requires its owned XDG profile.'
    }
    $State.sentinels = @(New-P4kxProfileSentinels $StockDirectory)
    Assert-P4kxDebArchive $After.asset $After.proof.version $Platform
    Invoke-P4kxLinuxTool sudo @('apt-get', 'install', '--yes', $After.asset) | Out-Null
    $new = Get-P4kxLifecycleDebState $After.proof $Platform
    $version = Invoke-P4kxPackageProcess '/usr/bin/p4kx' @('--version') $Directory (Join-Path $Directory 'candidate-version.log')
    Assert-P4kxLifecycleCandidateVersion $version $After.proof.version
    $State.baseline = $old
    $State.candidate = $new
}

function Invoke-P4kxFlatpakLifecycleReplacement {
    param($Before, $After, $Platform, [string]$Directory, [hashtable]$State)
    if ($Before.proof.flatpak.app_ref -cne $After.proof.flatpak.app_ref -or
        $Before.proof.flatpak.app_commit -ceq $After.proof.flatpak.app_commit -or
        $Before.proof.flatpak.runtime_ref -cne $After.proof.flatpak.runtime_ref -or
        $Before.proof.flatpak.runtime_commit -cne $After.proof.flatpak.runtime_commit) {
        throw 'Flatpak lifecycle requires distinct application commits under the same ref and runtime.'
    }
    Install-P4kxFlatpakRuntime $Before.proof $Platform
    Install-P4kxFlatpakBundle $Before.asset
    $old = Get-P4kxLifecycleFlatpakState $Before.proof $Platform
    $State.sentinels = @(New-P4kxProfileSentinels $old.profile)
    Install-P4kxFlatpakBundle $After.asset
    $new = Get-P4kxLifecycleFlatpakState $After.proof $Platform
    if ($new.profile -cne $old.profile) { throw 'Flatpak bundle replacement changed application profile identity.' }
    $id = (Get-P4kxLinuxPackaging).application_id
    $version = Invoke-P4kxPackageProcess flatpak @('run', '--user', "--arch=$($Platform.unix_arch)", '--command=p4kx', $id, '--version') $Directory (Join-Path $Directory 'candidate-version.log')
    Assert-P4kxLifecycleCandidateVersion $version $After.proof.version
    [void]$old.Remove('profile')
    [void]$new.Remove('profile')
    $State.baseline = $old
    $State.candidate = $new
}

function Assert-P4kxLifecycleCandidateVersion {
    param([string]$Output, [string]$Version)
    if (($Output -split '\r?\n')[0] -cne "p4kx $Version") { throw 'Upgraded package did not launch the candidate version.' }
}

function Invoke-P4kxLinuxLifecycleFormat {
    param([ValidateSet('deb','flatpak')][string]$Format, $Before, $After, $Platform,
        [string]$StockDirectory)
    Assert-P4kxDisposableLinuxRunner $Platform
    if (Test-P4kxLifecyclePackageInstalled $Format $Platform) { throw 'Lifecycle verification requires an unowned package identity.' }
    $directory = Join-Path $StockDirectory "lifecycle/$Format"
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    $state = @{sentinels=@()}
    $failures = [Collections.Generic.List[Exception]]::new()
    try {
        if ($Format -ceq 'deb') {
            Invoke-P4kxDebLifecycleReplacement $Before $After $Platform $StockDirectory $directory $state
        } else {
            Invoke-P4kxFlatpakLifecycleReplacement $Before $After $Platform $directory $state
        }
        Assert-P4kxProfileSentinels $state.sentinels
    }
    catch { $failures.Add($_.Exception) }
    finally {
        try {
            if (Test-P4kxLifecyclePackageInstalled $Format $Platform) { Remove-P4kxLinuxPackage $Format $Platform }
            if (Test-P4kxLifecyclePackageInstalled $Format $Platform) { throw 'Upgraded package ownership remains after removal.' }
        }
        catch { $failures.Add($_.Exception) }
        try { Assert-P4kxProfileSentinels $state.sentinels }
        catch { $failures.Add($_.Exception) }
    }
    if ($failures.Count) { throw [AggregateException]::new('Native Linux package lifecycle or cleanup failed.', $failures) }
    $state.baseline.version = $Before.proof.version
    $state.baseline.archive_sha256 = $Before.proof.archive_sha256
    $state.candidate.version = $After.proof.version
    $state.candidate.archive_sha256 = $After.proof.archive_sha256
    return @{
        schema=1; target=$Platform.target; revision=$After.proof.revision; package=$Format
        fixture_kind='same-source-lower-package-version'; baseline=$state.baseline; candidate=$state.candidate
        candidate_launch='success'; uninstalled='success'; profile_preserved='success'
    }
}

function Save-P4kxLinuxLifecycleEvidence {
    param([string]$Fixtures, [string]$Assets, [string]$Version, [string]$StockDirectory, $Platform, $Base)
    $fixtureVersion = Get-P4kxLinuxLifecycleFixtureVersion
    $failures = [Collections.Generic.List[Exception]]::new()
    $rows = @(foreach ($format in @('deb','flatpak')) {
        try {
            $before = Get-P4kxLinuxConsumerProducer $format $Fixtures $fixtureVersion $Platform $Base
            $after = Get-P4kxLinuxConsumerProducer $format $Assets $Version $Platform $Base
            if ($before.proof.archive_sha256 -ceq $after.proof.archive_sha256) {
                throw 'Lifecycle fixture and release package bytes must differ.'
            }
            Invoke-P4kxLinuxLifecycleFormat $format $before $after $Platform $StockDirectory
        }
        catch { $failures.Add($_.Exception) }
    })
    if ($failures.Count) { throw [AggregateException]::new('Required Linux package lifecycle evidence failed.', $failures) }
    $rows | ConvertTo-Json -Depth 8 -AsArray | Set-Content -LiteralPath (Join-Path $StockDirectory 'linux-lifecycle.json') -Encoding utf8NoBOM
}
