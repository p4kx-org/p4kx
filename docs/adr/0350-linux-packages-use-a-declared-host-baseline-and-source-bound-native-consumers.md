# Linux packages use a declared host baseline and source-bound native consumers

The Linux GUI, CLI and MCP server ship for AMD64 and ARM64 in tar.gz, AppImage, deb and Flatpak.
The user selected Ubuntu 24.04 / glibc 2.39 as the minimum native host baseline; Flatpak has the
separate Freedesktop Platform/SDK 25.08 ABI. `.config/platforms.json` remains the sole CPU/target
inventory, while `.config/linux-packaging.json` supplies the common reverse-DNS application ID
and sandbox policy. A private deb payload with public symlinks preserves real-executable update
identity without duplicate installations.

AppImage embeds a native runtime built from pinned source and static library inputs rather
than an opaque runtime asset with uncertain dependency versions. The release includes original
licences, complete corresponding library/runtime sources, recipes, patches, exact signed APK
inputs and build/relink instructions. A native static linker map rejects unrecorded archives.
Every format records actual post-transformation ELF digests, matching source commit/compiler/CPU
and unchanged application `.text` hashes; RPATH and stripping therefore remain auditable without
requiring byte-identical ELF files across formats.

A fresh matching native runner executes each actual package launcher and installation lifecycle
before release assembly. Flatpak uses narrowly scoped read-only prefix access and Documents
grants for selected inputs/outputs, with no host process execution or broad host write grant.
An unobserved host game process remains unknown inside its PID namespace. Real FUSE grants must
also establish installation-alias write refusal; a failing or unavailable required check cannot
produce a successful native proof. [Linux packages](../linux-packages.md) defines the complete
consumer, licensing and evidence contract, including the separate physical-device audit.
