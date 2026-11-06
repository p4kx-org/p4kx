param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$FixtureDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-installation.ps1')
. (Join-Path $PSScriptRoot 'flatpak-documents.ps1')
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
$documents = [Collections.Generic.List[string]]::new()
try {
    $source = Export-P4kxFlatpakDocument $archive $settings.application_id
    $documents.Add($source)
    $outside = Export-P4kxFlatpakDocument $destination $settings.application_id -Writable
    $documents.Add($outside)
    $alias = Export-P4kxFlatpakDocument $inside $settings.application_id -Writable
    $documents.Add($alias)
    foreach ($document in $documents) {
        $filesystem = (Invoke-P4kxLinuxTool stat @('--file-system', '--format=%T', $document) | Out-String).Trim()
        if ($filesystem -cne 'fuse' -and $filesystem -cne 'fuse.portal') { throw 'Document test is not using the portal FUSE filesystem.' }
    }
    $data = (Invoke-P4kxLinuxTool flatpak @('run', '--user', "--arch=$($platform.unix_arch)", '--command=printenv', $settings.application_id, 'XDG_DATA_HOME') | Out-String).Trim()
    $prefix = @('run', '--user', "--arch=$($platform.unix_arch)", '--command=p4kx',
        "--env=P4KX_FOLDERS_ROOT=$data/portal-smoke", '--env=P4KX_GAME_CHECK=off',
        "--env=P4KX_PROTECT_FOLDERS=$game", "--filesystem=${game}:ro", $settings.application_id)
    $null = Invoke-P4kxFlatpakDocumentCommand $prefix @('list', $source) $output (Join-Path $output 'read-only-list.log')
    $null = Invoke-P4kxFlatpakDocumentCommand $prefix @('extract', $source, '--out', $outside) $output (Join-Path $output 'portal-extract.log')
    $expected = (Get-FileHash -LiteralPath (Join-Path $FixtureDirectory 'hello.txt') -Algorithm SHA256).Hash
    if ((Get-FileHash -LiteralPath (Join-Path $destination 'Data/hello.txt') -Algorithm SHA256).Hash -cne $expected) { throw 'Portal output grant exported different bytes.' }
    $null = Invoke-P4kxFlatpakDocumentCommand $prefix @('extract', $source, '--out', (Join-Path $game 'direct-refused')) $output (Join-Path $output 'direct-refused.log') -Refused
    $null = Invoke-P4kxFlatpakDocumentCommand $prefix @('extract', $source, '--out', $alias) $output (Join-Path $output 'portal-refused.log') -Refused
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
