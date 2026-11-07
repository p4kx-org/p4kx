# MSI PATH ownership and bounded Environment notification

The per-user MSI must preserve the exact existence, Registry string kind and bytes of the user's
PATH across install, repair, upgrade, uninstall and rollback. Native adversarial package checks
show that the standard Environment-row insertion/removal cannot establish that contract. The
private `p4kx-msi-path` helper owns one explicit insertion and retains foreign edits instead of
normalising PATH or restoring an obsolete complete snapshot.

## Ownership and invocation isolation

A unique preexisting exact entry is borrowed and never removed. An inserted entry carries its
exact before/after proof. With an unchanged after-state, removing it restores the original
existence/type/bytes; after a foreign edit, removal splices only the unique exact owned entry and
one joining separator. Ambiguous duplicate entries fail without changing PATH.
Present values retain their exact terminator tail, including no terminator. Creating an absent
value chooses one explicit NUL-terminated Plain string.

The write-ahead journal binds each operation to ProductCode, the actual MSI PackageCode, declared
predecessor, typed goal and a fresh 256-bit invocation nonce from the operating system random
source (`getrandom::fill`). The captured sequence N identifies receipt N+1; the nonce isolates
separate invocations of the same package, including a later invocation with the same stale N.
Prepare, apply, commit and rollback require that complete binding. Random-source failure and an
all-zero result fail explicitly; there is no clock, counter or identifier fallback.

The safe `winreg` transactional API atomically compares and replaces the raw PATH, ownership,
journal, sequence and active invocation binding in HKCU's explicit 64-bit Registry view. There is no nontransactional
fallback. Actual Registry errors retain their typed original causes. The pure codec checks blob
limits before copying. Native reads check key metadata before winreg's allocation and the returned
value afterward; concurrent native allocation/isolation bounds require runner evidence.
Prepared state changes no PATH; applied state and its proof commit
together. A committed proof remains available to its matching MSI rollback callback if a later
commit action fails. Separate invocations cannot claim that terminal proof.

The private binary WAL uses version 3 and rejects incompatible versions, including versions 1
and 2. Prepared, Applied and
Committed bind the entire current owner, including its Borrowed or canonical Inserted claim, to
their original projection. RolledBack carries its source phase and exact observed PATH. One pure
compensation projection derives its complete durable owner, including legitimate borrowed or
reacquired claims. This historical proof does not constrain subsequent foreign PATH edits.

## Invocation reservation and transport

Reserve captures the complete immutable caller and base sequence N, generates one fresh nonce,
and atomically reserves that invocation without changing PATH, ownership or a terminal journal.
A later bound Prepare consumes only that reservation and builds receipt N+1 from the live PATH;
foreign PATH edits and predecessor cleanup before Prepare remain observable. Every mutation,
including terminal callback no-ops, refuses a foreign active reservation or cancellation fence.

The canonical binding is `v1|N|product32lowerhex|package32lowerhex|nonce64lowerhex`, at most
154 UTF16 words. Its Registry representation has exactly one NUL. The active Binding lives in
`Software\P4KX-Installer\PathOwnership\Invocation`; separating this small value from the large
WAL key keeps its 310-byte metadata bound meaningful. State, sequence, binding and PATH still
participate in one native transaction and full-image compare-and-replace. Torn or orphan fields
are rejected. Binding is transport, not authority independent of its complete stored caller.

Cancelling an unprepared reservation retains its exact binding as a cleanup-only fence at the
same N. Only the matching repeated cancel or rollback may become a no-op; Prepare, Apply and
Commit cannot reactivate it. A new reservation atomically replaces this fence with a fresh nonce.
No missing-state or product-only success is inferred.

Pending reservations fail closed by default. Explicit `recover-reserved` requires the exact
previous binding and replaces only its nonce through compare-and-replace. Product, Package,
goal, predecessor, N, owner, terminal journal and live PATH remain unchanged. Recovery cannot
reclaim a cancellation fence or Prepared/Applied journal; only active Reserved state is recoverable. If old Prepare wins the race, recovery
fails; if recovery wins, stale callbacks fail. No timeout or guessed process-liveness authority
is used.

The private EXE supports Reserve, captured-binding phases, explicit reserved recovery and the
fixed notification. Standard MSI AppSearch transport, actual SummaryInformation PackageCode
binding, major-upgrade callback exclusion and native lifecycle evidence must be completed before
release activation; the pure protocol does not establish those MSI integration facts.

## Approved notification exception

The owner explicitly approved only the fixed Windows notification leaf
`crates/p4kx-msi-path/src/notification_windows.rs`, against the reviewed candidate with SHA-256
`914C5378C5DFDB25939EB126BB8DDD4AE8670DF742FCD9378EE1F7FD1261AA05`.
This exception does not authorise unsafe Registry access, arbitrary MSI DLL exports or additional
native adapter leaves. The helper's library uses `#![forbid(unsafe_code)]`; only its Windows binary
leaf expects the lint. `cargo xtask lints` enforces both boundaries and the exact manifest lint
copy. ADR-0006's DWM exception remains independently scoped.

The leaf sends only `WM_SETTINGCHANGE` with wParam zero and a static NUL-terminated UTF16
`Environment` payload. At most 1024 observed top-level windows share one 1500-ms deadline,
including enumeration. Each send uses the remaining deadline, never `HWND_BROADCAST`; a recipient
on the current thread is rejected. Native errors, zero without a native detail, destroyed windows,
timeouts and incomplete enumeration remain distinct. A partial notification is an error, not a
success claim. The synchronous enumeration callback borrows only a live exclusive bounded
collection for the duration of `EnumWindows`.

## Integration evidence

The default-off `native-fixtures` feature exposes a fixed-scope raw Registry test seam and the
separately built `p4kx-msi-registry-fixture` Cargo example. It retains exact key/value existence,
types and bytes, uses the production WAL decoder and requires acknowledged source-bound hosted
runner context at capture and mutation boundaries. Both library and example roots must forbid
unsafe code; the lint gate rejects weaker annotations in either. Its transactional compare/read-back
contract does not establish native isolation or allocation bounds. The example is excluded from
release payloads; [its evidence contract](../msi-native-fixture.md) requires separate native proof.

Pure memory/codec tests establish state transitions and exact-byte preservation. Native signature
compilation establishes type compatibility only. Registry transactions and MSI lifecycle behavior,
including execution of the production notification path, require disposable AMD64 and ARM64 runner
proof; local developer machines execute none of those adapters. A release may use the helper only
after that native package evidence and session-to-receipt transport have been reviewed and passed.
These checks do not establish delivery to individual windows or hung-recipient behavior. A separate
message receiver is optional diagnostic infrastructure, not a release requirement.
