param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$FixtureDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-installation.ps1')
. (Join-Path $PSScriptRoot 'flatpak-documents.ps1')
. (Join-Path $PSScriptRoot 'portal-measurement.ps1')
$platform = Get-P4kxLinuxPlatform $Target
Assert-P4kxDisposableLinuxRunner $platform
$settings = Get-P4kxLinuxPackaging
$output = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $output) { throw 'Document portal smoke requires a new directory.' }
$game = Join-Path $output 'fake-game/LIVE'
$destination = Join-Path $output 'exports'
$inside = Join-Path $game 'inside-export'
New-Item -ItemType Directory -Force -Path (Join-Path $game 'Bin64'), $inside, $destination | Out-Null
$archive = Join-Path $game 'Data.p4k'
Copy-Item -LiteralPath (Join-Path $FixtureDirectory 'native-smoke.p4k') -Destination $archive
$archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash
@{
    target=$Target; revision=$env:GITHUB_SHA
    flatpak=(Invoke-P4kxLinuxTool flatpak @('--version') | Out-String).Trim()
    findmnt=(Invoke-P4kxLinuxTool findmnt @('--version') | Out-String).Trim()
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'portal-tools.json') -Encoding utf8NoBOM
$documents = [Collections.Generic.List[string]]::new()
try {
    $source = Export-P4kxFlatpakDocument $archive $settings.application_id
    $documents.Add($source)
    $outside = Export-P4kxFlatpakDocument $destination $settings.application_id -Writable
    $documents.Add($outside)
    $alias = Export-P4kxFlatpakDocument $inside $settings.application_id -Writable
    $documents.Add($alias)
    $service = 'org.freedesktop.portal.Documents'
    $object = '/org/freedesktop/portal/documents'
    $version = Invoke-P4kxPortalMeasurement busctl @('--user', '--no-pager', '--timeout=2', 'get-property', $service, $object, $service, 'version') $output 'documents-version'
    $introspection = Invoke-P4kxPortalMeasurement busctl @('--user', '--no-pager', '--timeout=2', '--xml-interface', 'introspect', $service, $object, $service) $output 'documents-introspection'
    $packages = Invoke-P4kxPortalMeasurement dpkg-query @('--show', '--showformat=${binary:Package}\t${Version}\n', 'xdg-desktop-portal', 'xdg-desktop-portal-gtk', 'flatpak', 'dbus') $output 'documents-packages'
    $measured = ConvertFrom-P4kxDocumentsInterface $version $introspection
    @{target=$Target; revision=$env:GITHUB_SHA; interface=$measured; packages=$packages.Trim()} |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $output 'documents-interface.json') -Encoding utf8NoBOM
    for ($index = 0; $index -lt $documents.Count; $index++) {
        $mount = Invoke-P4kxLinuxTool findmnt @('--kernel', '--json', '--list', '--output', 'TARGET,FSTYPE,SOURCE', '--target', $documents[$index]) | Out-String
        $mount | Set-Content -LiteralPath (Join-Path $output "portal-mount-$index.json") -Encoding utf8NoBOM
        Assert-P4kxFlatpakDocumentMount -Metadata $mount -MountPoint (Join-Path $env:XDG_RUNTIME_DIR 'doc')
    }
    $data = (Invoke-P4kxLinuxTool flatpak @('run', '--user', "--arch=$($platform.unix_arch)", '--command=printenv', $settings.application_id, 'XDG_DATA_HOME') | Out-String).Trim()
    $prefix = @('run', '--user', "--arch=$($platform.unix_arch)", '--command=p4kx',
        "--env=P4KX_FOLDERS_ROOT=$data/portal-smoke", '--env=P4KX_GAME_CHECK=off',
        "--env=P4KX_PROTECT_FOLDERS=$game", "--filesystem=${game}:ro", $settings.application_id)
    $null = Invoke-P4kxFlatpakDocumentCommand $prefix @('list', $source) $output (Join-Path $output 'read-only-list.log')
    $null = Invoke-P4kxFlatpakDocumentCommand $prefix @('extract', $source, '--filter', '*', '--out', $outside) $output (Join-Path $output 'portal-extract.log')
    $expected = (Get-FileHash -LiteralPath (Join-Path $FixtureDirectory 'hello.txt') -Algorithm SHA256).Hash
    if ((Get-FileHash -LiteralPath (Join-Path $destination 'Data/hello.txt') -Algorithm SHA256).Hash -cne $expected) { throw 'Portal output grant exported different bytes.' }
    $null = Invoke-P4kxFlatpakDocumentCommand $prefix @('extract', $source, '--filter', '*', '--out', (Join-Path $game 'direct-refused')) $output (Join-Path $output 'direct-refused.log') -Refused
    $null = Invoke-P4kxFlatpakDocumentCommand $prefix @('extract', $source, '--filter', '*', '--out', $alias) $output (Join-Path $output 'portal-refused.log') -Refused
    if ((Get-ChildItem -LiteralPath $inside -Force).Count -ne 0 -or (Test-Path -LiteralPath (Join-Path $game 'direct-refused'))) { throw 'A rejected export changed the game folder.' }
    $sourceWritable = @('run', '--user', "--arch=$($platform.unix_arch)", '--command=test', $settings.application_id, '-w', $source)
    $PSNativeCommandUseErrorActionPreference = $false
    & flatpak @sourceWritable
    if ($LASTEXITCODE -ne 1) { throw 'Read-only archive document grant is writable or could not be checked.' }
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -cne $archiveHash) { throw 'Document portal verification modified the source archive.' }
    @{target=$Target; revision=$env:GITHUB_SHA; read_only_archive='success'; writable_export='success'; game_alias_refusal='success'; source_unchanged='success'} |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'documents.json') -Encoding utf8NoBOM
}
finally { foreach ($document in $documents) { Invoke-P4kxLinuxTool flatpak @('document-unexport', $document) | Out-Null } }
