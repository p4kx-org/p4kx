# Native Documents-portal verification

Flatpak operational startup requires Documents interface 5 or newer. Native Linux package
verification uses two independent session buses on each supported CPU. Both sessions use the
same packaged application binaries on the Ubuntu 24.04/glibc 2.39 baseline.

The first session uses the stock services. It runs tar.gz, AppImage and deb checks, then proves
that all three real Flatpak applications reject API4 operational startup with
`[p4kx.runtime.documents-api5]`, the typed cause `[p4kx.documents.unsupported-interface:4]`
and exit 1. The generic prerequisite code alone is insufficient: session-bus, authentication,
access, timeout or malformed-reply failures do not establish an API4 refusal. Help and version remain usable; the GUI prerequisite
smoke uses `P4KX_SMOKE_FRAMES=3`. No operational profile, stored-key root or export may be created.
These negative checks are recorded as expected refusals and never satisfy runtime checks.

The second session activates the official source-built xdg-desktop-portal 1.22.1 Documents and
PermissionStore services. Its first service directory takes precedence over standard activation
directories. An independent abstract Unix listener uses EXTERNAL authentication; the normal
`XDG_RUNTIME_DIR/doc` FUSE mount is retained. The consumer records the authenticated daemon's
mountpoint bytes separately from the sandbox runtime directory, canonical `doc` path and kernel
mount inventory. A namespace alias does not require those path spellings to be identical.
No host-spawn capability or broad filesystem grant is added. The same installed Flatpak/OSTree payload runs all six existing CLI/MCP/GUI runtime
checks in this session.

Each session builds package-smoke launchers with its actual host output directory. The fixture
grant is read-only; the temporary writable grant covers that session's smoke directory, which
exists before the first application starts. Stock and API5 use the same launcher builder with
their respective output scopes. Host-side extracted-byte hashes remain required evidence.

The controller starts `dbus-daemon --nofork --print-address=1` directly, with `--session` for the
stock bus and the owned configuration for API5. It holds that exact process object and a separately
started PowerShell worker. Both inherit the same isolated XDG and Flatpak directories. The worker
receives exactly the measured Unix bus address; inherited D-Bus starter routes are removed.
Readiness has a five-second deadline and a bounded single address. Bus and worker output remain
separate and bounded throughout execution; an early bus exit fails the session.

The controller may stop only those two directly held processes with `Kill(false)` and verifies
their exit. It never signals a descendant tree. Activated services are distinct: only their
captured PID/start/executable identities are eligible for the pidfd cleanup path. Controller
exit evidence, final service inventory and the kernel mount barrier are all required before
another session. Timeouts, missing inventories and cleanup errors remain failures; cleanup does
not convert them to runtime evidence.

Both sessions require final process inventories and kernel mount disappearance before another
bus starts. Inventories retain the observed PID/start/executable identities from initial and final
snapshots. A failed teardown permits only bounded cleanup of captured identities through Linux
pidfds; it forbids another session. A cleanup-only third session is allowed after proved teardown
when package removal still needs to run. Its result cannot repair a failed main check or count as
runtime proof.

## Official test-service inputs

[The source recipe](../.config/documents-portal.json) pins the official release asset digest,
size, source commit and vendored libglnx/gvdb revisions. The builder keeps sandboxed validation
enabled and uses Meson `--wrap-mode=nodownload`; optional documentation, tests, geoclue, gudev,
systemd and Flatpak-interface targets are disabled. It builds only Documents and PermissionStore.

The native evidence records the exact recipe, observed compiler/tool/pkg-config/package versions,
license-source hashes and both executable ELF machines and hashes. Actual D-Bus owners and
`/proc/<pid>/exe` hashes must match that build. Version/introspection plus an empty typed
GetHostPaths call establish the real interface. These daemons are disposable test infrastructure;
they are not installed in the host's system service directories or shipped as P4KX payloads.

## Required real grant cases

| Case | Required observation |
| --- | --- |
| RO file-only Archive grant | Bin64 siblings remain hidden and the source is not writable. |
| Separate RW export grant | Actual extracted synthetic bytes equal the fixture. |
| Unknown source parent, empty protection list | Exit 2 with neutral `[p4kx.write.source-membership]`; no created output. |
| Absent output suffix | The same source-parent constraint applies before directory creation. |
| Covering directory grant with visible Bin64 | The existing known-installation refusal is proved separately. |
| Ordinary copied Archive directory | Observable absence of Bin64 allows normal output, including under its copy parent. |
| Host hardlink through another document grant | Explicit overwrite is refused as source-overwrite or target-identity uncertainty; source hash stays unchanged. |
| Forged underlying host-path xattr | Underlying value and observed host-FUSE value are recorded separately; normal source/target checks still run. |

The hardlink test compares actual host device/inode identities, records both FUSE proxy identities
and requires different document IDs. Proxy inequality never proves distinct host files. The test
uses `--conflict overwrite`, so the default skip policy cannot manufacture a refusal result.
The concrete leaf check uses the CLI's per-Entry failure status 1 and its JSON report: zero
extracted files, one typed failure for the selected Entry, no skipped/cancelled work and no
Archive change. Top-level refusals retain status 2. The case records the actual command exit
before classification and checks both linked files, an unrelated sentinel and the complete
output inventory even when classification fails.

The official 1.22.1 implementation synthesizes `user.document-portal.host-path`. Observing that
masking is not evidence that P4KX saw and ignored a visible forgery. The native report explicitly
keeps host-FUSE observations separate from product exposure; software tests cover untrusted xattr
handling independently.

## Evidence and gates

Documents evidence schema 2 binds both sessions, typed negative outcomes, actual service build,
all grant cases and teardown to the application source, native CPU and package SHA-256. The release
validator rejects skipped cases, owner/hash mismatches, mixed sessions, API4 results promoted to
runtime success and unsupported hardlink claims. Both main-session workers must exit zero;
completed teardown cannot turn a failed worker into runtime success. PowerShell process/configuration
mocks, direct-child toy controllers and injected pidfd tests establish orchestration behavior;
only the disposable native runner proves
real services, grants, kernel FUSE behavior and packaged application execution.

The API4 prerequisite subproof has schema 1. Each app retains its actual exit code, the
`unsupported-interface` reason and the typed interface version from the machine diagnostic.
The version must equal the independently measured integer API4 capability. Its capability owner
must match the Documents owner in the retained initial and final stock-session identities;
the negative record and both observations must name the same owned session. These are bounded
session observations, not a claim of a separate per-app owner measurement. Repeated identical
markers from the diagnostic's cause chain are allowed; malformed or mixed versions are refused.
The API5 positive cases and their runtime-success requirements remain independent.

Primary contracts: [official 1.22.1 release](https://github.com/flatpak/xdg-desktop-portal/releases/tag/1.22.1),
[official Meson inputs](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.1/meson.build),
[Documents FUSE implementation](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.1/document-portal/document-portal-fuse.c),
[D-Bus service-directory precedence](https://dbus.freedesktop.org/doc/dbus-daemon.1.html).
