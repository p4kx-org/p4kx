# Documents-portal identity core

The Linux Guard exposes `resolve_portal_alias(path, cancel)` as a synchronous identity
query. `NotPortal` means the observed path is outside the Documents mount. `Resolved`
contains an authenticated host spelling, document ID and file/directory grant kind.
`Unknown` retains a typed reason and local error source where available. None of these
results classifies a game installation or authorises a write. The installation/write
policy is separate; this core is not wired into `ensure_writable`.

A file-only `Data.p4k` grant can hide `Bin64` and its entire host parent. A successful
mapping does not establish sibling visibility, installation membership or a complete
protection set. An empty protection set cannot replace a portal identity query. A
directory mapping may append validated normal components for an output that does not
exist yet; it does not create that output or inspect an inaccessible host parent.

The resolver reads the kernel's `/proc/self/mountinfo`, selects the deepest covering
mount by component boundaries, and requires `fuse.portal` from source `portal`. The
Documents service's `GetMountPoint` must identify that exact mount. Grant object
device/inode identities and the complete covering mount row must remain equal before
and after the query. The original absolute input route is resolved again against a
fresh mount inventory and must still select the same target and mount, including
when an outer symbolic link reaches the document. Existing grant-root/descendant
symbolic links are uncertain.
Ordinary filesystem aliases are resolved through their nearest existing ancestor to
detect a route into the portal. These observations do not eliminate a later check/write
race or prove identity across another mount namespace.

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
fake-bus tests exercise the established client over loopback TCP and do not prove
Linux/FUSE behaviour. A Linux cross-check of the code is compilation evidence only.
The disposable native package consumer separately records actual Documents version,
XML introspection, package versions and mount metadata before exercising real grants.
The measurement has a 5 s process deadline and a shared 128 KiB UTF-8 stdout/stderr
budget. Its raw responses remain synthetic-runner artifacts; they are not printed as
GitHub metadata. Recording interface 4 does not itself change release policy.

Primary contracts: [Documents API](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Documents.html),
[mountinfo](https://man7.org/linux/man-pages/man5/proc_pid_mountinfo.5.html),
[zbus 5.19](https://docs.rs/zbus/5.19.0/zbus/), and
[busctl](https://www.freedesktop.org/software/systemd/man/latest/busctl.html).
