param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-source-tree.ps1')
if (-not $IsLinux) { throw 'Source-tree symlink preservation test requires native Linux.' }
$root = [IO.Path]::GetFullPath($Directory)
if ([IO.Path]::Exists($root)) { throw 'Source-tree test directory must be new.' }
$source = Join-Path $root 'source'
$destination = Join-Path $root 'copied'
New-Item -ItemType Directory -Path (Join-Path $source 'recipe/src') -Force | Out-Null
$original = Join-Path $source 'recipe/patch.diff'
[IO.File]::WriteAllBytes($original, [byte[]]@(0, 255, 13, 10, 42))
$external = Join-Path $root 'external-sentinel'
[IO.File]::WriteAllText($external, 'must never be dereferenced')
$links = [ordered]@{
    'recipe/src/relative.diff'='../patch.diff'
    'recipe/src/absolute.diff'='/export/sources/musl/distfiles/source.patch'
    'recipe/src/dangling.diff'='../missing.patch'
    'recipe/src/external.diff'=$external
    'recipe/linked-src'='src'
}
foreach ($name in $links.Keys) {
    Invoke-P4kxLinuxTool '/usr/bin/ln' @('--symbolic', '--', $links[$name], (Join-Path $source $name)) | Out-Null
}
Copy-P4kxLinuxSourceTree -Source $source -Destination $destination
if ((Get-FileHash -LiteralPath (Join-Path $destination 'recipe/patch.diff') -Algorithm SHA256).Hash -cne (Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash) {
    throw 'Corresponding source file bytes changed.'
}
foreach ($name in $links.Keys) {
    $copied = Get-Item -LiteralPath (Join-Path $destination $name) -Force
    if ($copied.LinkType -cne 'SymbolicLink' -or $copied.LinkTarget -cne $links[$name]) { throw "Source link was lost, dereferenced or changed: $name" }
}
$archive = Join-Path $root 'corresponding-source.tar.gz'
Invoke-P4kxLinuxTool tar @('--create', '--gzip', '--file', $archive, '--directory', $root, 'copied') | Out-Null
$unpacked = Join-Path $root 'unpacked'
New-Item -ItemType Directory -Path $unpacked | Out-Null
Invoke-P4kxLinuxTool tar @('--extract', '--gzip', '--file', $archive, '--directory', $unpacked, '--no-same-owner') | Out-Null
if ((Get-FileHash -LiteralPath (Join-Path $unpacked 'copied/recipe/patch.diff') -Algorithm SHA256).Hash -cne (Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash) {
    throw 'Corresponding-source tar changed regular source bytes.'
}
foreach ($name in $links.Keys) {
    $retained = Get-Item -LiteralPath (Join-Path $unpacked "copied/$name") -Force
    if ($retained.LinkType -cne 'SymbolicLink' -or $retained.LinkTarget -cne $links[$name]) { throw "Corresponding-source tar changed or dereferenced link: $name" }
}
$rejected = $false
try { Copy-P4kxLinuxSourceTree -Source $source -Destination $destination } catch { $rejected = $true }
if (-not $rejected) { throw 'Existing source-tree destination was overwritten.' }
$danglingDestination = Join-Path $root 'dangling-destination'
Invoke-P4kxLinuxTool '/usr/bin/ln' @('--symbolic', '--', 'missing-destination', $danglingDestination) | Out-Null
$rejected = $false
try { Copy-P4kxLinuxSourceTree -Source $source -Destination $danglingDestination } catch { $rejected = $true }
if (-not $rejected) { throw 'Dangling source-tree destination was overwritten.' }
'PASS: native source files, directory/relative/absolute/dangling/external symlinks and destination refusal. No external symlink payload dereferenced.'
