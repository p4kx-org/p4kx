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
