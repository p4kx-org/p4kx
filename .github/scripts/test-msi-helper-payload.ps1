param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-helper-payload.ps1')
if (Test-Path -LiteralPath $Directory) { throw 'Helper contract fixture directory must be new.' }
New-Item -ItemType Directory -Path $Directory | Out-Null
$system = Join-Path $Directory 'system/System32'
New-Item -ItemType Directory -Path $system | Out-Null
Set-Content -LiteralPath (Join-Path $system 'kernel32.dll') -Value 'Synthetic system PE identity; not executable.' -Encoding utf8NoBOM
$script:machine = 0
$script:imports = @('kernel32.dll')
function Get-ImageMachine { param([string]$Path,[string]$Os) if ($Os -cne 'windows' -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw 'Unexpected synthetic PE request.' }; return $script:machine }
function Get-P4kxVisualStudio { return 'synthetic-studio' }
function Get-P4kxDumpbin { param($Platform,[string]$Studio) if ($Studio -cne 'synthetic-studio') { throw 'Unexpected analyzer source.' }; return 'synthetic-analyzer-never-executed' }
function Get-P4kxPeDependencies { param([string]$Dumpbin,[string]$Path) if ($Dumpbin -cne 'synthetic-analyzer-never-executed') { throw 'Unexpected analyzer execution.' }; return $script:imports }
$priorSystem = [Environment]::GetEnvironmentVariable('SystemRoot')
$revision = 'a' * 40
$lockPath = Join-Path $Directory 'Cargo.lock'
'Synthetic dependency lock identity; not a Rust build.' | Set-Content -LiteralPath $lockPath -Encoding utf8NoBOM
$lockHash = (Get-FileHash -LiteralPath $lockPath -Algorithm SHA256).Hash.ToLowerInvariant()
$lockFailures = [Collections.Generic.List[string]]::new()
$count = 0
try {
    $env:SystemRoot = Join-Path $Directory 'system'
    foreach ($row in @(@('x86_64-pc-windows-msvc',34404),@('aarch64-pc-windows-msvc',43620))) {
        $script:machine = $row[1]
        $platform = [pscustomobject]@{target=$row[0];pe_machine=$row[1]}
        $folder = Join-Path $Directory $row[0]
        New-Item -ItemType Directory -Path $folder | Out-Null
        foreach ($name in @('p4kx-msi-path.exe','p4kx-msi-helper-NOTICE.txt','THIRD-PARTY-NOTICES.md')) { 'Synthetic contract bytes; not executable.' | Set-Content -LiteralPath (Join-Path $folder $name) -Encoding utf8NoBOM }
        $hash = (Get-FileHash -LiteralPath (Join-Path $folder 'p4kx-msi-path.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
        $sbom = @{bomFormat='CycloneDX';metadata=@{component=@{name='p4kx-msi-path';version='0.5.0';hashes=@(@{alg='SHA-256';content=$hash})};properties=@(@{name='p4kx:binary';value='p4kx-msi-path'},@{name='p4kx:role';value='embedded-msi-helper'},@{name='p4kx:target';value=$row[0]})};components=@(@{name='synthetic-dependency'})}
        $sbom | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $folder 'p4kx-msi-helper.cdx.json') -Encoding utf8NoBOM
        $notices = Join-Path $folder 'THIRD-PARTY-NOTICES.md'
        $inventory = @{schema_version=1;role='embedded-msi-helper';target=$row[0];version='0.5.0';revision=$revision;machine=$row[1];sha256=$hash;crt='static-msvc';rustflags='-Ctarget-feature=+crt-static';imports=@('kernel32.dll');compiler='rustc synthetic-contract';cargo_lock_sha256=$lockHash}
        foreach ($pair in @(@('sbom_sha256','p4kx-msi-helper.cdx.json'),@('helper_notice_sha256','p4kx-msi-helper-NOTICE.txt'),@('notices_sha256','THIRD-PARTY-NOTICES.md'))) { $inventory[$pair[0]]=(Get-FileHash -LiteralPath (Join-Path $folder $pair[1]) -Algorithm SHA256).Hash.ToLowerInvariant() }
        $baseline = $inventory | ConvertTo-Json -Depth 8
        $path = Join-Path $folder 'p4kx-msi-helper.json'
        $baseline | Set-Content -LiteralPath $path -Encoding utf8NoBOM
        Assert-P4kxMsiHelperPayload $folder $platform '0.5.0' $revision $notices $lockHash
        $lockCases = @(
            @{name='missing';mutate={param($v) $v.PSObject.Properties.Remove('cargo_lock_sha256')};expected=$lockHash},
            @{name='null';mutate={param($v) $v.cargo_lock_sha256=$null};expected=$lockHash},
            @{name='short';mutate={param($v) $v.cargo_lock_sha256='a'};expected=$lockHash},
            @{name='uppercase';mutate={param($v) $v.cargo_lock_sha256='A'*64};expected=$lockHash},
            @{name='paired-trailing-newline';mutate={param($v) $v.cargo_lock_sha256+="`n"};expected=($lockHash+"`n")},
            @{name='nonhex';mutate={param($v) $v.cargo_lock_sha256='g'*64};expected=$lockHash},
            @{name='foreign';mutate={param($v) $v.cargo_lock_sha256='f'*64};expected=$lockHash},
            @{name='number';mutate={param($v) $v.cargo_lock_sha256=1};expected=$lockHash},
            @{name='array';mutate={param($v) $v.cargo_lock_sha256=@('a'*64)};expected=$lockHash},
            @{name='object';mutate={param($v) $v.cargo_lock_sha256=[pscustomobject]@{sha256='a'*64}};expected=$lockHash},
            @{name='missing-expected';mutate={param($v)};expected=''},
            @{name='malformed-expected';mutate={param($v)};expected='wrong'},
            @{name='uppercase-expected';mutate={param($v)};expected='A'*64},
            @{name='foreign-expected';mutate={param($v)};expected='f'*64}
        )
        foreach ($case in $lockCases) {
            $changed=$baseline | ConvertFrom-Json
            & $case.mutate $changed
            $changed | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $path -Encoding utf8NoBOM
            $rejected=$false
            try { Assert-P4kxMsiHelperPayload $folder $platform '0.5.0' $revision $notices $case.expected }
            catch {
                if ($_.Exception.Message -notmatch 'Cargo\.lock') { throw }
                $rejected=$true
            }
            if (-not $rejected) { $lockFailures.Add("$($row[0])/$($case.name)") }
            $count++
        }
        foreach ($mutation in @(
            {param($v) $v.machine=0},{param($v) $v.target='foreign-target'},{param($v) $v.role='application'},
            {param($v) $v.revision='b'*40},{param($v) $v.version='0.6.0'},{param($v) $v.sha256='f'*64},
            {param($v) $v.sbom_sha256='f'*64},{param($v) $v.notices_sha256='f'*64},{param($v) $v.helper_notice_sha256='f'*64},
            {param($v) $v.crt='dynamic'},{param($v) $v.rustflags='-Ctarget-feature=-crt-static'},
            {param($v) $v.imports=@('vcruntime140.dll')},{param($v) $v.imports=@('kernel32.dll','kernel32.dll')},
            {param($v) $v.imports=@('api-ms-win-crt-runtime-l1-1-0.dll')},{param($v) $v.compiler=''}
        )) {
            $changed=$baseline | ConvertFrom-Json
            & $mutation $changed
            $changed | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $path -Encoding utf8NoBOM
            $rejected=$false
            try { Assert-P4kxMsiHelperPayload $folder $platform '0.5.0' $revision $notices $lockHash } catch { $rejected=$true }
            if (-not $rejected) { throw 'Malformed embedded helper provenance was accepted.' }
            $count++
        }
        foreach ($importCase in @(
            @{names=@('vcruntime140.dll')},@{names=@('api-ms-win-crt-runtime-l1-1-0.dll')},
            @{names=@('ucrtbase.dll')},@{names=@('unknown.dll')},@{names=@('../kernel32.dll')},
            @{names=@('kernel32.dll','kernel32.dll')},@{names=@()}
        )) {
            $rejected=$false
            try { Assert-P4kxMsiHelperImports $importCase.names $platform $system } catch { $rejected=$true }
            if (-not $rejected) { throw 'A dynamic CRT, unresolved, malformed or duplicate import was accepted.' }
            $count++
        }
    }
}
finally { [Environment]::SetEnvironmentVariable('SystemRoot',$priorSystem) }
if ($lockFailures.Count -gt 0) { throw ('Cargo.lock mutations accepted: ' + ($lockFailures -join ', ')) }
"PASS: both mocked CPU branches and $count provenance mutations; no analyzer, executable, Registry or MSI run."
