# Animation source export

`p4kx media export ARCHIVE --out DIR --kind animation` writes the shared
`animation.jsonl` stream. The source inventory retains the exact decoded Entry bytes
of selected CAF, DBA and AIM files through the normal RawReference. Filter expressions,
nested Archive traversal and atomic publication follow the [media export contract](media-export.md).

| Namespace | Indices | Payload |
| --- | --- | --- |
| `animation-entry` | none | Filename family, disposition and chunk count |
| `animation-chunk` | original chunk table ordinal | Numeric type, known type name, layout version, byte order, nullable chunk ID and Entry-relative extent |
| `animation-clip` | original clip ordinal, zero for CAF | Original 48 metadata bytes, full flag word, interpreted timing, physical ranges and DBA name |
| `animation-controller` | clip ordinal, controller ordinal | Original ID and both 12-byte channel declarations, with independent sample outcomes |

All ranges are half-open and relative to the original retained Entry. IVO chunk extents come
from the validated directory and may include padding. DBA controller keys can refer to a
shared pool beyond the data chunk's computed extent. The name interval includes the original
NUL byte. Invalid UTF-8 leaves its text view null while the raw name remains recoverable.
Duplicate or empty controller rows retain their own physical ordinals.

Each decoded channel has one `u16` little-endian chunk of original frame indices and one
`f32` little-endian chunk of decoded values. Values are grouped by key. Quaternion components
are x, y, z, w in CryEngine axes, normalized with continuous sign. Translations are x, y, z in
CryEngine axes and metres. Frame and value chunk references carry the source encoding intervals.
The original packed descriptor word has the time encoding in its low byte. Additive metadata
marks the stored clip semantics. The default inventory describes source clips independently.
Additive composition is outside this export.

AIM keeps its raw bytes and chunk table with the `opaque-aim` disposition. An invalid container,
malformed clip or unreadable declared channel produces an explicit issue. A readable sibling
channel and subsequent structurally identifiable clips remain in a diagnostic generation.
`--diagnostic` preserves those rows without replacing a complete index. The provider processes
one retained Entry at a time and writes decoded arrays one channel at a time. It shares the
export's source, record, chunk and output budgets. Clip and controller inventories follow authored
order in full.

The field-origin companion addresses every JSON field. Decoded chunk receipts bind the actual
source descriptor and encoded interval to each emitted artifact occurrence. A source-backed
field from an encrypted Entry is marked independently of the Entry name or suffix derived
from the Archive directory.

## Installed Archive verification

The opt-in Media census selects every CAF, DBA and AIM Entry across the root and nested
containers. Each family uses a separate temporary generation. It verifies physical selection,
raw bytes, chunk and clip order, original controller words, declared and decoded key counts,
and the typed sample bytes and source ranges. It reconciles the counts with the header reader
where that reader covers the selected source. Legacy header-budget refusals are counted
separately. The streaming source reader still checks the complete available declarations.
Diagnostic rows are verified and an incomplete census fails the check.

Configure the Archive paths and temporary directory as described in [CLAUDE.md](../CLAUDE.md).
The test uses `P4KX_DATA_P4K` and the optional `P4KX_DATA_P4K_B`, then reports aggregate
counts without recording asset names or payloads.

```powershell
cargo test --locked -p p4kx-media --test real_archive all_selected_animation_entries_clips_and_keys_reconcile -- --ignored
```

## Optional skeleton bindings

`p4kx media export Data.p4k --kind animation --animation-skeleton Data/Objects/Rig.chr --out export`
requests one exact relative Entry path in each selected CAF or DBA's immediately owning
Archive. The lookup uses the Archive's normal path matching and keeps physical duplicates
ambiguous. A nested Archive resolves the same path independently. The command reads an
include-filtered dependency when it proves one usable candidate. Explicit exclusions keep the
skeleton unread. An existing generation follows `--conflict` as usual. Use a fresh output
directory or request replacement to add bindings to a previous export.

`animation-skeleton` records the directory reference and skeleton disposition. A ready
skeleton supplies the original chunk and bone count. Missing, duplicate, excluded and
unsupported targets retain their explicit disposition. Unreadable inputs, failed validation
and configured limits follow the shared media error and diagnostic policies. One selected
skeleton is parsed and retained per active owning Archive and the shared input budgets apply.

`animation-controller-binding` is a separate row at the same clip and controller ordinals.
It keeps the first authored controller, the first key-bearing occurrence, the first matching
bone and the number of matching bones. Only the first key-bearing controller binds the first
matching bone. Empty occurrences remain available with no bound bone. Key codec errors remain
on the original controller row. Binding metadata describes numeric ID selection and supplies
no additive composition or sample conversion.

The skeleton contributes to fields that use its actual controller table or selected bone
ID. Controller occurrence ordinals and decoded sample artifacts keep their animation
origin. Existing raw controller and sample rows are independent of the binding request.
