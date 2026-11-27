$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'native-evidence.ps1')
. (Join-Path $PSScriptRoot 'binary-machine.ps1')

function Get-P4kxApplicationBinaryEvidence {
    param([Parameter(Mandatory)][string]$Directory, [Parameter(Mandatory)]$Platform)
    $expectedMachine = Get-P4kxNativeMachine -Platform $Platform
    foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
        $filename = if ($Platform.os -ceq 'windows') { "$name.exe" } else { $name }
        $path = Join-Path $Directory $filename
        $file = Get-Item -LiteralPath $path
        if ($file -isnot [IO.FileInfo]) { throw "Expected executable file: $path" }
        $image = if ($Platform.os -ceq 'macos') {
            if ($file.LinkType) { throw "Expected regular signed bundle executable: $path" }
            Assert-P4kxMachOImage -Path $path -Platform $Platform -Kind Executable
        } else {
            $machine = Get-ImageMachine -Path $path -Os $Platform.os
            if ($machine -ne $expectedMachine) {
                throw "$name has machine $machine, expected $expectedMachine for $($Platform.target)."
            }
            [ordered]@{ machine = $machine }
        }
        $image.name = $name
        $image.sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        $image
    }
}

function Assert-P4kxMatchingApplicationBinaries {
    param(
        [Parameter(Mandatory)][object[]]$Actual,
        [Parameter(Mandatory)][object[]]$Expected,
        [Parameter(Mandatory)]$Platform
    )
    Assert-P4kxBinaryEvidence -Metadata @{ binaries = $Actual } -Platform $Platform
    Assert-P4kxBinaryEvidence -Metadata @{ binaries = $Expected } -Platform $Platform
    foreach ($binary in $Actual) {
        $match = @($Expected | Where-Object name -CEQ $binary.name)
        if ($match.Count -ne 1) { throw "Missing unique producer evidence for $($binary.name)." }
        foreach ($field in $binary.Keys) {
            if ($match[0].$field -cne $binary[$field]) {
                throw "Packaged $($binary.name) differs from its producer's signed bytes or Mach-O identity."
            }
        }
    }
}
