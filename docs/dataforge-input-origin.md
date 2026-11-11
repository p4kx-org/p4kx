# DataForge input origin

The Extraction service retains the physical input of successfully read DataForge bytes.
`DataForgeEntry::origin()` and `LoadedDataForge::origin()` expose `DataForgeOrigin`:

| Variant | Retained evidence | Original P4K encryption |
| --- | --- | --- |
| `Archive(EntryOrigin)` | Actual root-input binding, ordered containing Entries and leaf Entry metadata | Fold of supported source declarations and all ancestors |
| `File(DataForgeFileOrigin)` | Actual absolute input path, opened-handle `FileStamp`, and shared read identity | Unknown without verified original provenance |
| `Unknown` | An unbound generic stream | Unknown |

Identical decoded bytes from an Archive and a standalone file retain different origin knowledge.
An additional Archive supplied for text lookup does not establish that an external database came
from that Archive. A display filename or path-like stream label does not establish physical origin.

## Archive ownership and containing Entries

`Archive::entry_origin(&Entry)` requires the actual borrowed indexed slot of that Archive. A foreign
Entry or detached clone returns `None`, even if every numeric field and path is equal. The returned
owned snapshot retains the root-input token, root-to-parent ancestor steps and leaf metadata after
the Archive is dropped. Container evidence is captured when the Container opens; it does not depend
on selecting or emitting that ancestor as an output row.

Each step exposes its admitted Entry index, stored path and original `EntryMetadata`. Source offsets
remain relative to the immediately owning Archive source. A decoded compressed Container does not
turn its inner offsets into physical outer-file offsets. The index-based key-demand identities stay
independent of the new evidence.

`ArchiveInputOrigin::is_same_input` compares a shared in-process observation token. Containers share
one token. Independently opened inputs have different tokens even when their declarations look equal.
`Archive::owns_origin` additionally checks the complete known containing-index chain; sibling
containers are not interchangeable. `DataForgeOrigin::matches_archive` uses that contract only for
Archive origins. File and Unknown require separate input-kind handling; false alone does not describe
why an origin failed to match. Tokens are not serialized, host-path hashes or persistent game IDs.

## Encryption interpretation

`EntryOrigin::encryption()` returns `SourceEncryption::Encrypted`, `Unencrypted` or `Unknown`.
Valid supported CIG/v2 word 1 establishes encrypted origin. Valid v2 word 0 establishes unencrypted
origin. Valid CIG word 0 establishes unencrypted origin only when the retained ZIP encryption bits
0 and 6 are both clear. Unsupported ZIP flags, missing/malformed/conflicting CIG metadata and unknown
v2 words remain unknown. The existing `Entry::is_encrypted()` decoder policy is unchanged.

Any proven encrypted contributing ancestor or leaf makes the combined origin encrypted, including
when another step is unknown. All steps must be known unencrypted to establish unencrypted origin.
Otherwise it remains unknown. This is source-declaration evidence, not proof of successful decryption
or an Archive-wide health/key verdict. Failed reads create no successful DataForge contributor;
their admitted Entry evidence remains available independently.

## File and worker lifetime

Standalone reads bind an absolute input path before opening and use that same path for the post-read
check. `FileStamp` reuses the opened handle's size and optional modification observation; it is not a
cryptographic attestation or proof of P4K history. `DataForgeFileOrigin::is_same_input` compares handles
of the same observed read. Its Debug output omits the host path and opaque identity.

The existing worker queue retains origin beside the canonical database. `LoadedDataForge::with`
keeps its existing calling contract and uses the same dispatch as `with_origin`, whose closure receives
both the database and its captured origin. Cached `Arc<LoadedDataForge>` handles preserve the same
context. No separate worker or source lookup reconstructs it from the current Archive or key state.

## Canonical read ownership

`RecordTree::is_from(&Record)` checks the original borrowed DataForge owner and physical
Record position retained by its actual TreeBuilder read. Equal GUIDs, indices, paths or
values from another DataForge do not satisfy it. A genuine whole-tree clone keeps the
binding. Tree value equality remains independent from that runtime authority.

Each actual `Field` retains `FieldStorage`, available through `Field::storage()`. Its
`data_type`, `is_array` and raw `target` word describe the declaration that decoded that
same field. A WeakPointer can resolve to the same visible Reference value as a stored
Reference GUID; this receipt preserves their original declaration difference. The target
word is not a resolved target name. Copying this descriptive receipt cannot attach it to
a different field or establish physical input authority.

`Reference::original_target()` returns a borrowed `ReferenceOrigin` from the original
Record or weak-pointer admission. `shares_source_with` checks its DataForge owner, not
that the supplied Record is the target. The view exposes the actual target Record index,
GUID and normalized path, plus the instance location when applicable. `record_type` is
inapplicable for an owned-instance target; callers must not invent its owner Record type
from the instance type or reconstruct a stored filename from the normalized path.
Reference value equality, Debug and canonical JSON/XML keep their descriptive contract;
none serializes or restores the private original owner.

For an owned-instance target, `ReferenceOrigin::instance_positions()` borrows the exact
original field and array ordinals, including inherited fields. These positions distinguish
physical fields with identical names; the logical location string cannot do that. Record
targets return `None`. Public `Location::new` does not create an original receipt. The
existing target cache retains one tuple per admitted instance, shared by Reference clones.
Location equality, hashing, Debug, Display and canonical JSON/XML keep their semantic
steps and text; equal positions alone never establish original ownership.

The tuple adds one optional boxed-slice descriptor per Location and four payload bytes per
original ordinal for an admitted instance, plus allocator overhead. No additional source
read, target lookup or per-Reference allocation is introduced. Direct Location cloning
copies its tuple; whole Reference clones continue to share the existing Location.
These types are reexported by `p4kx-extraction`. They add no second source lookup or
Record decode, per-field allocation, Archive dependency in DataForge, or new data limit.
Their Rust memory layout is target dependent. An output adapter must still pair the
actual tree and owner, capture the same field's value and declaration, and register the
actual payload/selector/conversion contributions before publishing an origin marker.

## Observed loadout documents

`LoadoutFiles::read_observed` returns one `LoadoutFileRead` pairing its actual bytes or
failure with the original `LoadoutFileSource`. `ArchiveFiles` captures the actual admitted
Entry before size/open/read failures. A generic provider remains unbound and is read once;
its descriptive `identity` cannot bind arbitrary bytes to an Archive. Forwarding the
whole genuine read preserves the original pair.

`LoadoutFileObservation` retains that operation's exact read outcome and optional actual
parse outcome. `LoadoutReadEvent::FileReused` borrows those original outcomes without a
second read, parse or read/parse counter increment. A context budget refusal can leave
the intrinsic parse outcome absent while reporting incomplete inventory. A cache event
alone does not supply the current caller's authored selection contributions.

The Export adapter retains genuine file-read and projection proofs for the resolver cache's
lifetime while draining each caller's ledger delta. Reuse validates the original admission,
intrinsic document kind and exact read/optional parse outcomes; it emits no new file operation
or duplicate diagnostic text. Archive admission follows the original input/container/Entry
binding; a generic or missing source retains its existing opaque handle. Equal path/index
descriptions and separately created unbound sources do not establish the same admission.

Read, projection and reuse events retain the same readonly original document-operation handle.
Cloning it is not another IO operation; two genuine cold reads of the same Entry remain distinct.
The adapter records observed read/projection phases separately from the receipt's intrinsic
outcomes. A later-filled parse result alone supplies no missing callback proof. Projection must
use its own observed successful read and may complete after another operation; delivery order
and matching file metadata cannot replace the actual operation handle. Repeating a read or
projection callback for that same handle is an explicit accounting error, with no extra row.

Observer cancellation and unavailable original admission remain fatal across later reads,
cache accesses and resolver roots. The Export adapter preserves the first cause;
`CatalogError::LoadoutSourceEvidence` retains actual admission unavailability rather than
turning it into an ordinary unreadable source row. Normal no-observer reads retain their
existing route and do not create original-source observation storage.

### Authored requests and direct XML Includes

`LoadoutPath` retains path text and its optional authentic selection as one private pair.
Cloning that pair preserves the selection; copying its text cannot bind a new path to the
original getter. Record-path callbacks run within the actual Record/tree projection, and XML
callbacks borrow the original document operation and the actual node of the same parse. The
XML site includes the real attribute name, decoded value and decoding outcome. A selection
handle alone does not replace these source-read proofs.

FileRead and FileProjection carry the original candidate together with the current caller.
FileDependency reports one selected direct Include, with both genuine parent and child
operation handles, the child's unchanged first-population candidate, and the actual Include
request. It follows the child's actual read and optional projection. Missing or malformed
children retain their real failed outcomes. FileDependencyRefused instead carries the genuine
parent, selected Include and actual error when a cycle or input policy refuses the request
before a child operation exists. It never invents a read or Entry for the refused target.

Local XML cache hits reference the original child operation through that one direct edge.
Global XML cache reuse emits only one FileReused root link with today's caller; consumers retain
the original dependency graph and follow its existing edges. Vehicle-cache reuse keeps its
separate flat set of original candidate operations. Neither route repeats source reads or
parses. An observer stop prevents later operations and successful edge/reuse callbacks.

The direct graph avoids repeatedly expanding a shared dependency closure for every Include.
A seven-document complete DAG has seven reads, seven projections and twenty-one direct edges;
its later root-cache hit adds one reuse link and no new edge. Operation storage and callback
count follow the actual documents and selected edges, rather than the number of transitive
paths. Existing XML parse-depth and file-count policies remain separate and unchanged.

An output adapter must validate both edge endpoints against their actual observed phases and
expected original input, retain the genuine Include recipe, and connect the current caller
through the root link before publishing derived field origins. An independently observed
unbound provider remains explicitly unknown/incomplete; a known Archive input must still
supply its actual ArchiveInputOrigin. Missing implementation capture cannot use the unbound
case as a fallback. These SDK receipts do not by themselves complete every output adapter.

## Export contract and verification

The [shared source-contribution API](source-contributions.md) defines runtime inventory authority,
byte domains, exact field addresses and explicit bounded readers for serialized descriptions.

This API preserves evidence for [per-data-value output origins](adr/0357-source-encryption-provenance-is-explicit-per-data-value.md).
It does not itself add the required field markers to every tool serializer. #1125 owns those shared
carriers, actual payload/selector/conversion uses, source references and immediate nullable
`containsEncryptedSource` values. A row/global key notice cannot substitute for field origin.

Synthetic tests exercise foreign/cloned same-index Entries, distinct containing positions under one
root, dropped Archives, unknown and encrypted ancestors, ordinary ZIP/malformed declarations,
missing/wrong/correct keys, unchanged duplicate key-demand identity, external files with identical
bytes, generic streams, failed reads and actual worker/cache lifetime. No real game data is required.
