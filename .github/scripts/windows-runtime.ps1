$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'binary-machine.ps1')

function Get-P4kxVisualStudio {
    $locator = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    $path = & $locator -latest -products '*' -requiresAny -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 Microsoft.VisualStudio.Component.VC.Tools.ARM64 -property installationPath
    if ($LASTEXITCODE -ne 0 -or -not $path) { throw 'Visual Studio C++ tools are required for dependency analysis.' }
    return $path.Trim()
}

function Get-P4kxPeDependencies {
    param([string]$Dumpbin, [string]$Path)
    $output = & $Dumpbin /nologo /dependents $Path
    if ($LASTEXITCODE -ne 0) { throw "PE dependency analysis failed: $Path" }
    return @($output | ForEach-Object { if ($_ -match '^\s+([A-Za-z0-9_.-]+\.dll)\s*$') { $Matches[1].ToLowerInvariant() } })
}

function Assert-P4kxWindowsRuntimeInventory {
    param([string]$Directory, $Platform)
    $inventory = Get-Content (Join-Path $Directory 'windows-runtime.json') -Raw | ConvertFrom-Json
    if ($inventory.target -cne $Platform.target) { throw 'Windows runtime inventory target mismatch.' }
    $expected = @($inventory.redistributables.name | Sort-Object)
    $present = @(Get-ChildItem -LiteralPath $Directory -File -Filter '*.dll' | Select-Object -ExpandProperty Name | Sort-Object)
    if (($expected -join ',') -cne ($present -join ',')) { throw 'Runtime DLL files and inventory differ.' }
    foreach ($dll in $inventory.redistributables) {
        if ($dll.name -cnotmatch '^[a-z0-9_.-]+\.dll$' -or $dll.source -cnotmatch '^Visual Studio/VC/Redist/MSVC/') { throw 'Unsafe or missing runtime DLL source identity.' }
        $path = Join-Path $Directory $dll.name
        if ($dll.machine -ne $Platform.pe_machine -or (Get-ImageMachine -Path $path -Os windows) -ne $dll.machine -or
            (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $dll.sha256) {
            throw 'Runtime DLL machine or digest differs from its inventory.'
        }
    }
    if (-not (Test-Path -LiteralPath (Join-Path $Directory 'MICROSOFT-REDISTRIBUTABLE-NOTICE.txt'))) { throw 'Runtime redistribution notice is missing.' }
}

function Get-P4kxCrtDirectory {
    param(
        [Parameter(Mandatory)][string]$Redistributables,
        [Parameter(Mandatory)][ValidateSet('x64', 'arm64')][string]$Architecture
    )
    $version = Get-ChildItem -LiteralPath $Redistributables -Directory |
        Where-Object { $_.Name -cmatch '^\d+\.\d+\.\d+$' } |
        Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
    if (-not $version) { throw 'No Microsoft C++ redistributable inventory was found.' }
    $target = Join-Path $version.FullName $Architecture
    if (-not (Test-Path -LiteralPath $target -PathType Container)) {
        throw "Missing target C++ redistributable directory: $target"
    }
    $candidates = @(Get-ChildItem -LiteralPath $target -Directory | Where-Object {
        $_.Name -cmatch '^Microsoft\.VC\d+\.CRT$' -and
        -not ($_.Attributes -band [IO.FileAttributes]::ReparsePoint)
    })
    if ($candidates.Count -ne 1) {
        throw "Expected one regular target CRT directory in $target; found $($candidates.Count)."
    }
    return $candidates[0]
}

function Copy-P4kxWindowsRuntime {
    param([string]$Directory, $Platform)
    $studio = Get-P4kxVisualStudio
    $hostCpu = [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString().ToLowerInvariant()
    $tools = Join-Path $studio 'VC/Tools/MSVC'
    $version = Get-ChildItem -LiteralPath $tools -Directory | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
    $dumpbin = Join-Path $version.FullName "bin/Host$hostCpu/$($Platform.msi_arch)/dumpbin.exe"
    if (-not (Test-Path -LiteralPath $dumpbin)) { throw "Missing host PE analyzer: $dumpbin" }
    $redistributables = Join-Path $studio 'VC/Redist/MSVC'
    $crt = Get-P4kxCrtDirectory -Redistributables $redistributables -Architecture $Platform.msi_arch
    $crtSource = [IO.Path]::GetRelativePath($redistributables, $crt.FullName).Replace('\', '/')
    $pending = [Collections.Generic.Queue[string]]::new()
    foreach ($file in Get-ChildItem -LiteralPath $Directory -Filter '*.exe') { $pending.Enqueue($file.FullName) }
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $inventory = [Collections.Generic.List[object]]::new()
    while ($pending.Count -gt 0) {
        $image = $pending.Dequeue()
        foreach ($dependency in Get-P4kxPeDependencies $dumpbin $image) {
            if (-not $seen.Add($dependency)) { continue }
            if ($dependency -match '^(vcruntime|msvcp|concrt)140.*\.dll$') {
                $source = Join-Path $crt.FullName $dependency
                if (-not (Test-Path -LiteralPath $source)) { throw "Missing target CRT dependency: $source" }
                $machine = Get-ImageMachine -Path $source -Os windows
                if ($machine -ne $Platform.pe_machine) { throw "Wrong CPU in CRT dependency: $source" }
                Copy-Item -LiteralPath $source -Destination $Directory
                $pending.Enqueue((Join-Path $Directory $dependency))
                $inventory.Add([ordered]@{
                    name=$dependency; machine=$machine; sha256=(Get-FileHash $source -Algorithm SHA256).Hash.ToLowerInvariant()
                    source="Visual Studio/VC/Redist/MSVC/$crtSource/$dependency"
                })
            }
            elseif ($dependency -notmatch '^(api-ms-win-|ext-ms-)' -and
                -not (Test-Path -LiteralPath (Join-Path $env:SystemRoot "System32/$dependency"))) {
                throw "Unresolved non-system runtime dependency: $dependency"
            }
        }
    }
    [ordered]@{
        target=$Platform.target; analyzer=@{role='host PE import analysis'; executable='Microsoft dumpbin'; host_cpu=$hostCpu}
        windows_runtime='Windows system DLLs and Universal CRT'; redistributables=@($inventory)
    } | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Directory 'windows-runtime.json') -Encoding utf8NoBOM
    @'
Microsoft Visual C++ runtime redistribution

Any Microsoft runtime DLLs listed in windows-runtime.json are unmodified files from
the architecture-specific Microsoft Visual Studio C++ redistributable directories
recorded by their source paths in that inventory.
Copyright Microsoft Corporation. All rights reserved.
Their use and redistribution remain subject to the applicable Visual Studio terms:
https://visualstudio.microsoft.com/license-terms/
Redistributable list and deployment documentation:
https://learn.microsoft.com/en-us/visualstudio/releases/2022/redistribution
https://learn.microsoft.com/en-us/visualstudio/releases/2026/redistribution
https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files

The inventory records each bundled file's source, PE machine and SHA-256 digest.
Windows operating-system DLLs and the Universal CRT are OS prerequisites and are
not copied from the build machine. Rust dependency licences are in
THIRD-PARTY-NOTICES.md. Host packaging tools are not shipped with P4KX.
'@ | Set-Content (Join-Path $Directory 'MICROSOFT-REDISTRIBUTABLE-NOTICE.txt') -Encoding utf8NoBOM
}
