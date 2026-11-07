param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-installation.ps1')
$root = [IO.Path]::GetFullPath($Directory)
if (Test-Path -LiteralPath $root) { throw 'MSI invocation test directory must be new.' }
New-Item -ItemType Directory -Path $root | Out-Null
& (Join-Path $PSScriptRoot 'test-msi-path.ps1') -Directory (Join-Path $root 'path-contract')
& (Join-Path $PSScriptRoot 'test-msi-path-probe.ps1') -Directory (Join-Path $root 'path-probe-contract')
$payload = Join-Path $root 'payload with spaces/Ü雪/package.msi'
New-Item -ItemType Directory -Path (Split-Path -Parent $payload) -Force | Out-Null
[IO.File]::WriteAllText($payload, 'synthetic MSI path fixture, never installed')
$processDirectory = Join-Path $root 'different process directory'
New-Item -ItemType Directory -Path $processDirectory | Out-Null
$location = Get-Location
$nativeDirectory = [IO.Directory]::GetCurrentDirectory()
try {
    Set-Location -LiteralPath $root
    [IO.Directory]::SetCurrentDirectory($processDirectory)
    $resolved = Resolve-P4kxMsiInput 'payload with spaces/Ü雪/package.msi'
    if ($resolved -cne (Get-Item -LiteralPath $payload).FullName -or ($IsWindows -and $resolved.Contains('/'))) {
        throw 'MSI input did not resolve to the native absolute file from the PowerShell location.'
    }
    $log = Join-Path $root 'logs with spaces/installer.log'
    $start = New-P4kxMsiexecStartInfo -Arguments @('/i', $resolved) -Log $log
    $expected = @('/i', $resolved, '/qn', '/norestart', '/l*v', $log)
    if ($start.FileName -cne 'msiexec.exe' -or $start.UseShellExecute -or -not $start.CreateNoWindow -or
        $start.WindowStyle -ne [Diagnostics.ProcessWindowStyle]::Hidden -or $start.ArgumentList.Count -ne $expected.Count) {
        throw 'MSI invocation lost its isolated quiet process contract.'
    }
    for ($index = 0; $index -lt $expected.Count; $index++) {
        if ($start.ArgumentList[$index] -cne $expected[$index]) { throw 'MSI invocation changed an argument boundary.' }
    }
    $productCode = '{11111111-2222-4333-8444-555555555555}'
    $uninstall = New-P4kxMsiexecStartInfo -Arguments @('/x', $productCode) -Log $log
    if ($uninstall.ArgumentList[1] -cne $productCode) { throw 'MSI uninstall changed the product identity.' }
    foreach ($invalid in @('missing.msi', 'payload with spaces/Ü雪')) {
        $rejected = $false
        try { Resolve-P4kxMsiInput $invalid | Out-Null } catch { $rejected = $true }
        if (-not $rejected) { throw 'Missing or directory installer input was accepted.' }
    }
    if (Test-Path -LiteralPath $log) { throw 'The invocation contract launched an installer.' }
}
finally {
    [IO.Directory]::SetCurrentDirectory($nativeDirectory)
    Set-Location -LiteralPath $location.Path
}

function New-Object {
    param([string]$ComObject)
    if ($ComObject -cne 'WindowsInstaller.Installer') { throw 'Unexpected COM object in enumeration regression.' }
    return [pscustomobject]@{Kind='synthetic installer'}
}

function Get-P4kxComProperty {
    param($Object, [string]$Property, [object[]]$Arguments)
    switch ($Property) {
        'RelatedProducts' { return [pscustomobject]@{Kind='synthetic related products'} }
        'Count' { return $script:registeredProducts.Count }
        'Item' { return $script:registeredProducts[[int]$Arguments[0]] }
        default { throw 'Unexpected COM property in enumeration regression.' }
    }
}

$identities = @('{11111111-2222-4333-8444-555555555555}', '{66666666-7777-4888-8999-000000000000}')
foreach ($count in 0..2) {
    $script:registeredProducts = @($identities | Select-Object -First $count)
    $actual = @(Get-P4kxInstalledProducts '{aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee}')
    if ($actual.Count -ne $count) { throw 'MSI related products were wrapped or dropped.' }
    $iterations = 0
    foreach ($product in Get-P4kxInstalledProducts '{aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee}') {
        if ($product -cne $identities[$iterations]) { throw 'MSI product enumeration changed identity or order.' }
        $iterations++
    }
    if ($iterations -ne $count) { throw 'MSI cleanup would run for a nonexistent product or miss a product.' }
}
$predecessor = [pscustomobject]@{registry=@(
    @('1', 'Software\KRT\P4K-Extractor', 'Application'),
    @('1', 'Software\KRT\P4K-Extractor', 'StartMenuShortcut'),
    @('2', 'Software\KRT\P4K-Extractor', 'UnrelatedMachine'),
    @('1', 'Software\Other', 'UnrelatedUser')
)}
$keys = @(Get-P4kxPredecessorRegistryKeys $predecessor)
if ($keys.Count -ne 1 -or $keys[0] -cne 'Software\KRT\P4K-Extractor') { throw 'Predecessor ownership key selection differs.' }
$rejected = $false
try { Get-P4kxPredecessorRegistryKeys ([pscustomobject]@{registry=@(@('1', 'Software\P4K-Extractor', 'WrongNamespace'))}) | Out-Null } catch { $rejected = $true }
if (-not $rejected) { throw 'An unsupported predecessor registry namespace was accepted.' }
'PASS: native MSI paths, spaces/Unicode, distinct PowerShell/process directories, quiet argv, missing-input refusal and zero/one/many product enumeration; no installer executed.'
