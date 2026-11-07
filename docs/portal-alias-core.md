# Documents-portal identity core

The Linux Guard exposes `resolve_portal_alias(path, cancel)` as a synchronous identity
query. `NotPortal` means the observed path is outside the Documents mount. `Resolved`
contains an authenticated host spelling, document ID and file/directory grant kind.
`Unknown` retains a typed reason and local error source where available. None of these
results classifies a game installation or authorises a write. A retained local cause is
the original concrete error in the standard error chain, including after cloning a failure.
The installation/write
policy is separate. `ensure_writable` authenticates target routes, and Archive-derived writers
retain an `ArchiveWriteProtection` context for their actual source Archives.

A file-only `Data.p4k` grant can hide `Bin64` and its entire host parent. A successful
mapping does not establish sibling visibility, installation membership or a complete
protection set. An empty protection set cannot replace a portal identity query. A
directory mapping may append validated normal components for an output that does not
exist yet; it does not create that output or inspect an inaccessible host parent.

The resolver reads the kernel's `/proc/self/mountinfo`, selects the deepest covering
mount by component boundaries, and requires `fuse.portal` from source `portal`. The
Documents service's unmodified `GetMountPoint` bytes are bound to that exact visible
mount through its existing canonical directory. This permits the runtime `doc` symlink
without treating a lexical path substitution as authentication. The declared route is
observed twice against fresh inventories; probe and mapping use the same private binding.
Grant object device/inode identities and the complete covering mount row must remain equal before
and after the query. The original absolute input route is resolved again against a
fresh mount inventory and must still select the same target and mount, including
when an outer symbolic link reaches the document. Existing grant-root/descendant
symbolic links are uncertain.
Ordinary filesystem aliases are resolved through their nearest existing ancestor to
detect a route into the portal. The accepted route retains the original grant/suffix
components after its alias prefix enters the Documents mount, so their symbolic links
remain visible to the grant checks. Foreign nested mounts remain uncertain. These
observations do not eliminate a later check/write race or prove identity across another
mount namespace.

Only the official Documents `GetHostPaths` API is an origin authority. It is available
from interface version 5. Version 4, a missing method, denied access, timeout, owner
change, invalid signature, duplicate IDs and missing mappings remain `Unknown`. No
user xattr, `Info`/`List` fallback, host-spawn command or wider filesystem permission
contributes identity. Kernel mount metadata alone is not host-origin evidence.

The transport uses the existing zbus 5.19 blocking client with internal async-io. A
same-user Unix peer, EXTERNAL authentication and the local session-bus/proxy's owner
authority are trust assumptions; this is not executable attestation. The service's
unique owner is queried before and after the transaction, calls target that unique
owner, and successful replies must carry the expected sender. Error text from remote
services is discarded after typed classification, so it cannot copy private host
paths into public error chains.

Linux can report peer PID 0 when the peer is outside the caller's PID namespace. The
kernel credential UID still has to equal the caller's effective UID; PID visibility is
not authentication authority. The safe nix credential reader preserves PID 0 as a value.
The zbus client uses EXTERNAL authentication without its separate P2P-server credential
path. [The upstream rustix report](https://github.com/bytecodealliance/rustix/issues/1698)
documents why a credential representation requiring a nonzero PID is unsuitable here.

The limits have different scopes:

| Limit | Scope |
| --- | --- |
| 512 KiB / 4096 rows | Kernel mount inventory |
| 16 KiB | A pathname, and each accepted application reply body before our semantic decoding |
| 128 MiB | zbus's native binary message frame maximum, before its frame allocation |
| 500 ms | Each blocking D-Bus method call |
| 2 s | Unix connection, authentication and the complete D-Bus transaction |
| 10 ms | Cancellation supervisor polling interval |

The 16 KiB application limit does not precede zbus's binary frame handling or its
internal error decoding. Its authentication buffer has no independently configured
small byte cap. The local session bus is therefore trusted for that transport boundary.
Filesystem operations are synchronous and do not acquire a separate hard I/O deadline.
The transaction supervisor shuts down the actual socket on expiry/cancellation and
is joined on success, error and timeout; no detached timeout thread remains.

Unit tests use safe zbus message builders and actual local sockets, without a global
service. Linux-specific blocked-read and fake-bus tests use Unix streams. Windows
fake-bus tests exercise safe local Unix sockets and do not prove Linux/FUSE behaviour. A Linux
cross-check of the code is compilation evidence only.
The disposable native package consumer separately records actual Documents version,
XML introspection, package versions and mount metadata before exercising real grants.
Each measurement has a 5 s process deadline and normally a shared 128 KiB UTF-8
stdout/stderr budget. Mountpoint and sandbox runtime-path responses are capped at
16 KiB; the kernel inventory uses a 512 KiB cap with one overflow sentinel byte.
The consumer records the host Flatpak version, owner-bound `GetMountPoint`, actual
sandbox `XDG_RUNTIME_DIR`, its canonical `doc` path and sandbox mount inventory.
Its raw responses remain synthetic-runner artifacts; they are not printed as
GitHub metadata. Flatpak startup requires interface 5 and a successful authenticated capability
probe; recording interface 4 is evidence for the required negative session, not acceptance.

## Source-bound write policy

Each Archive job retains the actual source identity and observes installation membership;
comparison reports retain both Archives. Read-only previews can retain write uncertainty for a
later write decision once runtime prerequisites have passed. A file-only grant with an invisible
parent cannot establish the absence of `Bin64`. Source membership uncertainty is distinct from a
known installation. An empty discovered protection set grants no authority.

Authenticated host spellings identify namespaces. Different FUSE proxy device/inode pairs do not
prove different host files. Existing unowned targets whose host identity cannot be excluded remain
refused, including a second grant containing a hardlink to the Archive. Ordinary non-portal
sources additionally use stable native handles and canonical source routes.

`create_output_file` checks the job before exclusive `create_new` and retains the original file
descriptor in a sealed `OwnedFileToken`. The token binds content writes to that actual file,
authenticated host spelling and the same private job identity. It does not waive source membership,
original-source-name or known-installation checks. Writer-handle duplication failure uses verified
cleanup. If initial descriptor identity cannot be acquired, `RetainedCreation` identifies the
retained filename and original failure; it does not silently retry or delete an unverified file.

`OwnedDirectoryPermit` grants only namespace operations on an exclusively created staging root.
`ensure_namespace_writable` does not authorise content writes. Replacement roots require domain
validation between stable root observations. Rename rebinds the same root; cleanup checks source
names, authenticated containment and root identity without following links. Unlinking an owned
namespace entry does not modify the bytes of another hardlink target. No manifest path list,
`.old` suffix or caller-supplied spelling creates ownership authority.

Directory roots are authenticated directly, including a selected directory-grant root.
Entry rename/unlink checks then stay within that bound visible and host namespace. Cleanup
revalidates the root instead of requesting write authority for virtual document-ID parents.
Existing ancestors remain route/type observations; only missing concrete descendants are
created through the protected directory factory.

Pure cancellation is distinct from failed cleanup, retained output or failed restoration.
Ordinary authenticated cleanup can complete after cancellation; portal uncertainty still refuses
cleanup. These checks do not eliminate every later filesystem race or establish identity across
arbitrary mount namespaces. Native real-FUSE verification remains a separate release gate.

Raw Extraction, converted textures/audio, Record files, Query results, atomic output helpers and
single-file Record exports retain their active descriptor tokens through publication or explicit
cleanup. `owned_output_failure` recovers typed creation failures through factory I/O wrappers;
retained creation is terminal even when its original cause has `AlreadyExists` kind. Cancellation
does not suppress a separate retained-output or cleanup failure, and unpublished Record rows are
not counted as written.

Catalog-file callers use `write_catalog_file` or `write_catalog_file_from`. Their factory remains
lazy until publication health is accepted. The concrete file remains available on serializer failure
for explicit cleanup; a failed cleanup carries both the original catalog error and its independent
cause. Callers cancelling or rejecting an Archive watch after successful serialization explicitly
call `CatalogFile::discard` before returning. `Drop` is an emergency cleanup attempt, not the regular
error-reporting path.

The [native verification guide](portal-native-verification.md) describes the disposable API4/API5
sessions, actual FUSE grants and remaining evidence requirements. Windows synthetic descriptor tests
and Linux cross compilation do not replace that runtime evidence.

Primary contracts: [Documents API](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Documents.html),
[mountinfo](https://man7.org/linux/man-pages/man5/proc_pid_mountinfo.5.html),
[zbus 5.19](https://docs.rs/zbus/5.19.0/zbus/), and
[busctl](https://www.freedesktop.org/software/systemd/man/latest/busctl.html).
