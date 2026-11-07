param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'native-package-task.ps1')
$root = [IO.Path]::GetFullPath($Directory)
if ([IO.Path]::Exists($root) -or [IO.DirectoryInfo]::new($root).LinkTarget) { throw 'Native package path test directory must be new.' }
New-Item -ItemType Directory -Path $root | Out-Null
$cached = Join-Path $root 'target/build-tool-output-contract-check'
New-Item -ItemType Directory -Path $cached -Force | Out-Null
$sentinel = Join-Path $cached 'cached-sentinel.txt'
[IO.File]::WriteAllText($sentinel, 'retained Cargo cache fixture')
$temporary = Join-Path $root 'runner temporary'
$unique = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach ($platform in (Get-P4kxPlatformContract).targets) {
    foreach ($role in @('producer', 'consumer')) {
        foreach ($identity in @(@{run='123'; attempt='1'}, @{run='123'; attempt='2'}, @{run='124'; attempt='1'})) {
            $path = New-P4kxNativePackageTask -Directory $temporary -RunId $identity.run -Attempt $identity.attempt -Target $platform.target -Role $role
            if (-not $unique.Add($path) -or -not $path.StartsWith($temporary + [IO.Path]::DirectorySeparatorChar, [StringComparison]::Ordinal)) {
                throw 'Native task identity collided or escaped RUNNER_TEMP.'
            }
            $rejected = $false
            try { New-P4kxNativePackageTask -Directory $temporary -RunId $identity.run -Attempt $identity.attempt -Target $platform.target -Role $role | Out-Null } catch { $rejected = $true }
            if (-not $rejected) { throw 'A reused native task directory was accepted.' }
        }
    }
}
if ([IO.File]::ReadAllText($sentinel) -cne 'retained Cargo cache fixture') { throw 'Cached build state was changed.' }
foreach ($arguments in @(@{Directory='relative'}, @{RunId='../escape'}, @{Attempt='0'}, @{Target='foreign-target'})) {
    $values = @{Directory=$temporary; RunId='125'; Attempt='1'; Target=(Get-P4kxPlatformContract).targets[0].target; Role='producer'}
    foreach ($key in $arguments.Keys) { $values[$key] = $arguments[$key] }
    $rejected = $false
    try { New-P4kxNativePackageTask @values | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Invalid native task identity was accepted.' }
}
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
foreach ($workflow in @('release-linux.yml', 'release-windows.yml')) {
    foreach ($line in Get-Content -LiteralPath (Join-Path $repository ".github/workflows/$workflow")) {
        if ($line -match '\btarget/' -and -not $line.Contains('target/$env:P4KX_TARGET/release')) {
            throw "Non-Cargo native package state remains in cached target/: $workflow"
        }
    }
}
"PASS: $($unique.Count) unique run/attempt/target/role roots, repeat refusal, cached-state preservation and workflow path isolation."
