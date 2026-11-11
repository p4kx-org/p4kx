# Native macOS support in v0.5.0

This specification extends the [v0.5.0 implementation plan](https://github.com/p4kx-org/p4kx/issues/1115)
and the [release gate](https://github.com/p4kx-org/p4kx/issues/1116). It defines required release
acceptance. Delivery requires the evidence below.

## Supported target

- Ship `p4kx` (CLI), `p4kx-gui` (GUI) and `p4kx-mcp` (stdio) for Apple Silicon,
  `aarch64-apple-darwin`, with macOS 26 Tahoe as the minimum. Declare the deployment target in
  the build and the minimum OS in the app bundle. Verify the actual packaged Mach-O architecture
  and deployment target for every shipped executable and any bundled libraries.
- Exercise the v0.5.0 workflows on the minimum OS and the current stable macOS release. P4KX
  runs as a native ARM64 process. Wine and CrossOver supply optional game installations.
  Standalone Archives and manually selected installations work without either runtime.
- Keep Archive browsing, Comparison, Extraction, conversions, Catalog and Media Exports, and
  read-only MCP operations available through their existing entry points and shared contracts.
  Verify the native Blender 5.2 LTS add-on installation and import of a synthetic P4KX package on
  Apple Silicon. Check the hierarchy, materials, textures and a representative rig and animation
  against fixture expectations.

## Discovery and desktop behavior

- Extend the shared bounded Channel and launcher-source discovery for Wine and CrossOver. Read
  the standard Wine prefix (`~/.wine`), explicit `WINEPREFIX` and P4KX-configured roots, standard
  CrossOver bottle locations and configured CrossOver bottle roots. Document the supported
  settings and precedence. Finder launches must discover standard locations and saved settings.
  Keep manual folder and Archive selection available.
- Reuse the admitted prefix and launcher-log sources for Channels, public patches and the Guard.
  Test default and custom libraries, drive mapping, duplicate/aliased sources, cache invalidation,
  missing sources and partial discovery. Read only the existing permitted launcher sources.
  Retain the protection of discovered and manually selected game installations and Archives.
- Detect a running Star Citizen process under Wine or CrossOver using native macOS process
  evidence and the shared Guard states and warnings. Verify running, stopped and unavailable
  observation cases. A launcher or unrelated Wine process alone is insufficient evidence that
  the game is running. Cover protected paths through aliases, symlinks and different case
  spellings according to the actual volume, including write attempts through existing aliases.
- Exercise the shared macOS profile, data and cache locations, native open/save dialogs,
  Command-key shortcuts, system clipboard, Metal rendering and CoreAudio playback (start, seek
  and stop). Cover absent audio devices, cancelled dialogs, permission denial, Unicode paths,
  case-sensitive and case-insensitive APFS, and Archives on external volumes.

## DMG, signing and updates

- Publish one ARM64 DMG on GitHub Releases. It contains a relocatable `P4KX.app` with all three
  native programs, resources and notices. Support Finder drag-to-install and launch. Document
  CLI invocation and the executable path for an MCP client. Verify these paths after relocating
  the bundle or launching through a user-owned symlink. Normal use requires neither developer
  tools nor a package manager.
- Use Developer ID Application signing with hardened runtime and Apple notarization when
  available. Validate the signature and stapled ticket on the final DMG and check Gatekeeper
  behavior on a clean Mac. If Developer ID or notarization is unavailable, publish an ad-hoc-signed
  DMG, identify that mode and its reason in release evidence, and describe the first-launch
  approval in the Release notes. Report a signing or notarization validation failure as a failure.
  PR package dry runs must work with ad-hoc signatures and without signing secrets.
- Store the bounded distribution identity in `Contents/Resources/p4kx-distribution.json`. All
  three binaries use the shared update selector with their running target and package identity.
  Cover a compatible DMG, a missing or incompatible asset, malformed identity, and Release-page
  fallback. Keep the update check opt-in and link-only.

## Native acceptance and release evidence

- Extend the one platform contract in `.config/platforms.json` and its existing build, cache,
  nextest, runtime and release consumers. Keep the pinned compiler and test runner, add Mach-O
  checks alongside PE/ELF checks, and reject wrong-target, wrong-host or wrong-source artifacts.
  Run the required synthetic test suites, doctests, lints and repository checks on native macOS.
- On each supported verification OS, launch the packaged CLI for synthetic Archive list,
  Extraction and conversion, use MCP initialize, tools/list and a representative read-only
  request, and start, draw and exit the GUI. Verify DMG mounting, Finder installation, bundle
  replacement on upgrade, removal and profile preservation on a clean host. Test the selected
  signing mode on the quarantined download with default Gatekeeper policy.
- Record OS version, CPU, Mach-O identities, exact source commit, tool pins, binary hashes, DMG
  hash and successful check results. Use suitable physical Macs when hosted environments cannot
  establish Metal, audio, clipboard, dialog, Wine/CrossOver or Finder behavior. Record synthetic
  CI and device evidence separately. Cross-compilation, emulation and skips cannot satisfy
  required native checks. Real Archive evidence stays local or explicitly private.
- Final v0.5.0 preflight requires five native targets, thirteen native packages, fifteen
  target/application SBOMs and five target-bound runtime records, including the macOS minimum and
  current OS evidence. Include the DMG in existing checksum, notice, provenance, attestation and
  scan gates for the exact published bytes. The Blender add-on retains its separate asset and
  provenance. Update the supported-platform guide, installation help, Release notes and changelog
  when implementation and evidence are ready.

## References

- [Shared native platform contract (ADR-0343)](adr/0343-one-native-platform-contract-and-target-bound-runtime-evidence.md)
- [Read-only installations (ADR-0036)](adr/0036-read-only-towards-the-installation.md)
- [Launcher discovery and protection](channel-discovery.md)
- [Rust macOS targets](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html)
- [GitHub native runner images](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [Apple bundle layout](https://developer.apple.com/documentation/bundleresources/placing-content-in-a-bundle)
- [Apple distribution and notarization](https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution)
- [Apple Gatekeeper first-launch behavior](https://support.apple.com/en-us/102445)
- [CrossOver bottle directories](https://support.codeweavers.com/en_US/change-the-bottle-directory-in-crossover-mac)
