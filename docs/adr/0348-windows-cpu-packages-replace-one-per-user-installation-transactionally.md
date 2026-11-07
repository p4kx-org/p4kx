# ADR-0348: Windows CPU packages replace one per-user installation transactionally

Windows AMD64 and ARM64 packages replace one per-user P4KX installation. They share UpgradeCode
`090C8F25-8543-42F0-B40A-1514653149F9`, `%LOCALAPPDATA%\Programs\P4KX`, one Start menu shortcut,
one user PATH entry and the installer markers in `HKCU\Software\P4KX`. Each MSI build gets fresh
product/package codes; ARM64 components have different GUIDs from the preserved AMD64 components.
Parallel installations with competing PATH entries would make the selected program ambiguous.

An explicitly chosen same-version package can replace the other CPU on Windows ARM64. The
shared updater selects the running binary's CPU and package kind; it only provides a link.
An AMD64 compatibility installation on ARM64 is never native ARM64 runtime evidence. Windows
Installer rejects an ARM64 MSI on AMD64 through its package platform metadata.

Removal of the previous product follows InstallInitialize, inside the transaction, before the
new files are installed. A failed upgrade must restore the original product, payload and PATH.
The CI rollback probe is a separately identified, modified copy of the next MSI that executes
its installation script and fails before InstallFinalize. The shipped MSI has no test action.
The profile and `archive.key` remain outside MSI ownership, and survive upgrades, CPU
replacement and uninstall. Portable execution uses an extracted folder without installer state.
The embedded Rust helper owns the exact PATH insertion and transfers its claim during modern
replacement; old-product removal skips the helper while the replacing invocation remains bound.
[ADR-0354](0354-msi-path-ownership-and-bounded-environment-notification.md) defines that protocol.

Both target packages are executed on matching native hosted runners, with source commit,
compiler, nextest, PE machines, payload hashes and package hashes recorded. Shipped release
EXEs receive the CLI/MCP/GUI smokes directly. Host-only WiX, dumpbin and CycloneDX execution
establishes package/dependency metadata and is recorded separately from native app execution.
Runtime DLLs come from the matching Microsoft redistributable directory and have an inventory
and licence notice. No DLL search-path expansion is introduced.

References: [WiX upgrade scheduling](https://docs.firegiant.com/wix/schema/wxs/majorupgrade/),
[MSI platform template](https://learn.microsoft.com/en-us/windows/win32/msi/template-summary),
[InstallExecute transaction](https://learn.microsoft.com/en-us/windows/win32/msi/installexecute-action),
[InstallFinalize transaction](https://learn.microsoft.com/en-us/windows/win32/msi/installfinalize-action).
