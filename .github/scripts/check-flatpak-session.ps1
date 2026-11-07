param(
    [Parameter(Mandatory)][ValidateSet('Api5', 'Cleanup')][string]$Mode,
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$FixtureDirectory,
    [Parameter(Mandatory)][string]$StatePath,
    [Parameter(Mandatory)][string]$ServiceBuildMetadata,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-consumer.ps1')
. (Join-Path $PSScriptRoot 'flatpak-installed-identity.ps1')
. (Join-Path $PSScriptRoot 'documents-portal-interface.ps1')
. (Join-Path $PSScriptRoot 'documents-portal-session.ps1')
$platform = Get-P4kxLinuxPlatform $Target
Assert-P4kxDisposableLinuxRunner $platform
$role = if ($Mode -ceq 'Api5') { 'api5' } else { 'cleanup-only' }
Assert-P4kxOwnedDocumentsSession $role
$build = Get-Content -LiteralPath $ServiceBuildMetadata -Raw | ConvertFrom-Json
$failures = [Collections.Generic.List[Exception]]::new()
$preserved = @()
try {
    if ($Mode -ceq 'Api5') {
        $saved = Get-Content -LiteralPath $StatePath -Raw | ConvertFrom-Json -AsHashtable
        if ($saved.target -cne $Target -or $saved.revision -cne $env:GITHUB_SHA) { throw 'Flatpak session state source differs.' }
        $state = $saved.state
        $null = Get-P4kxLiveFlatpakIdentity $platform $saved.producer
        $data = (Invoke-P4kxLinuxTool flatpak @('run', '--user', "--arch=$($platform.unix_arch)", '--command=printenv', (Get-P4kxLinuxPackaging).application_id, 'XDG_DATA_HOME') | Out-String).Trim()
        if ($data -cne $state.data_directory) { throw 'Flatpak application profile identity changed between independent sessions.' }
        $images = @(Get-P4kxLinuxApplicationEvidence $state.binaries $platform)
        Assert-P4kxLinuxPackageExecution @{target=$Target; package='flatpak'; binaries=$images} $saved.producer $platform
        $interface = Get-P4kxDocumentsInterfaceEvidence $OutputDirectory 5
        Save-P4kxSandboxMountEvidence $platform $OutputDirectory
        Save-P4kxDocumentsSessionIdentity $OutputDirectory $build 'initial'
        $preserved = @(New-P4kxProfileSentinels (Join-Path $state.data_directory 'preserved-profile'))
        $smoke = Join-Path $OutputDirectory 'smoke'
        $launchers = Get-P4kxFlatpakLaunchers $platform $FixtureDirectory $smoke $state.data_directory
        & (Join-Path $PSScriptRoot 'check-package-runtime.ps1') -Target $Target -BinaryDirectory $state.binaries -FixtureDirectory $FixtureDirectory -OutputDirectory $smoke -Package flatpak -Launchers $launchers -ExpectedVersion $Version
        $result = Get-Content -LiteralPath (Join-Path $OutputDirectory 'smoke/runtime.json') -Raw | ConvertFrom-Json
        Assert-P4kxLinuxPackageExecution $result $saved.producer $platform
        & (Join-Path $PSScriptRoot 'check-flatpak-documents.ps1') -Target $Target -FixtureDirectory $FixtureDirectory -DataDirectory $state.data_directory -OutputDirectory (Join-Path $OutputDirectory 'documents')
        $documents = Get-Content -LiteralPath (Join-Path $OutputDirectory 'documents/documents.json') -Raw | ConvertFrom-Json
        $documents | Add-Member -NotePropertyName api5_interface -NotePropertyValue $interface
        $result | Add-Member -NotePropertyName documents -NotePropertyValue $documents
        $result | Add-Member -NotePropertyName producer -NotePropertyValue $saved.producer
        $result | Add-Member -NotePropertyName archive_sha256 -NotePropertyValue $saved.producer.archive_sha256
        $result | ConvertTo-Json -Depth 24 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'package.json') -Encoding utf8NoBOM
    }
}
catch { $failures.Add($_.Exception) }
finally {
    try {
        $applications = @(Invoke-P4kxLinuxTool flatpak @('list', '--user', '--app', '--columns=application'))
        if ((Get-P4kxLinuxPackaging).application_id -cin $applications) { Remove-P4kxLinuxPackage 'flatpak' $platform }
        @{mode=$Mode; outcome='success'; runtime_pass=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'package-cleanup.json') -Encoding utf8NoBOM
    }
    catch { $failures.Add($_.Exception) }
    try { Assert-P4kxProfileSentinels $preserved } catch { $failures.Add($_.Exception) }
    try { Save-P4kxDocumentsSessionIdentity $OutputDirectory $build } catch { $failures.Add($_.Exception) }
}
if ($failures.Count -ne 0) { throw [AggregateException]::new('API5 package/session or cleanup failed.', $failures) }
