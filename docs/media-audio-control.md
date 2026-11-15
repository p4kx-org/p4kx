# Audio control source export

`p4kx media export ARCHIVE --out DIR --kind audio-control --filter '*.xml'` exports selected
XML Entries below `Data/Libs/GameAudio/`, including nested Archive scopes. The path test accepts
both separator spellings and ASCII case variants. The normal filter still controls which raw
Entries are retained. Audio-control works independently of WEM and BNK semantic decoding.
Add `--kind audio` when physical Wwise Event candidates should be available.

Each source keeps its original verified raw bytes and physical Entry identity. The JSONL stream
uses flat rows and exact original text byte ranges or CryXml table coordinates. Byte ranges are
half-open and include UTF-8 BOM offsets. Child rows keep parent and sibling ordinals. Original
text may include adjacent lexical pieces such as entity spellings and CDATA. CryXml node text
is an independent slot alongside the child order.

| Namespace | Physical keys |
| --- | --- |
| `audio-control-document` | Source Entry and empty indices |
| `audio-control-element` | Source Entry and original traversal occurrence |
| `audio-control-declaration` | Element occurrence and original declaration ordinal |
| `audio-control-content` | Original traversal occurrence of text, comment or instruction |
| `audio-control-binary-text` | Element occurrence of the independent CryXml content slot |
| `audio-control-definition` | Element occurrence |
| `audio-control-field` | Element occurrence and original declaration ordinal |
| `audio-control-reference` | Element occurrence and original declaration ordinal |
| `audio-control-reference-resolution` | Owning Archive directory and query index |
| `audio-control-reference-candidate` | Owning Archive directory, query index and candidate ordinal |
| `audio-control-path-candidate` | Source Entry, declaration key and candidate ordinal |
| `audio-control-failure` | Source Entry and empty indices |

The original element and declaration rows preserve exact qualified names, case, authored order,
namespace declarations, decoded values and available literal spellings. Unknown elements and
attributes remain there regardless of typed recognition. Known source families include triggers,
Wwise Events, RTPCs, switches and states, environments, preloads, groups, platform choices,
dependees and external sources. Field rows cite the original declaration. Numeric facts include
authored Wwise duration and radius bounds, RTPC multiplier and shift, and
`WwiseRtpc` min, max and default. External-source rows retain the authored
localisation flag, entry language, duration declarations and Wwise filename
as separate fields. The raw declaration keeps its original literal independently of the typed number. An invalid typed
literal keeps its original spelling and an explicit typed invalid outcome. The bounds are authored
metadata and give no runtime timing, unit or playback guarantee.

A preload link searches original named requests in the same Archive directory. An RTPC dependee
searches original RTPC declarations there. A platform group link searches within its exact
owning preload request. Links retain every physical candidate, including repeated declarations.
The export writes authored edges without following them, so reciprocal edges remain visible.
An incomplete directory or unread potential target keeps scope completeness explicit.

Wwise Event links keep the authored name and the Wwise ShortID and can join the owning directory's
Audio `audio-bank-foreign-target-resolution` by query index. With Audio omitted the link reports
`not-selected`. Full authored Archive file paths can be checked against physical directory
metadata. Relative external filenames retain their original spelling and an
explicit unresolved path state. Exact duplicate entries are emitted in order with independent filter/key information.
An unsafe or unproved relative spelling retains the original token and an explicit reason. The
path lookup consumes only directory metadata. Runtime availability belongs to playback.

Each field has immediate source-use receipts and an independent encryption marker. A declaration
from a protected ATL source keeps that proof, and a reference to other bytes does not establish
that those bytes contributed. Common generation budgets govern actual input and output work.
Malformed XML and repaired CryXml string-table bytes produce diagnosed incomplete output when
requested with `--diagnostic`. The original raw source remains separately recoverable.

## Installed ATL checks

Both checks use the standard `P4KX_DATA_P4K` and Archive key settings. The
authored-field witness reports the opened Build and walks root ATL XML Entries
in physical order. It skips other XML roots after verification and stops once
all required declaration groups are observed. An exhausted source scope
reports the missing groups by name.

```powershell
cargo test --locked -p p4kx-media --lib sampled_original_atl_families_keep_their_exact_authored_values_and_origins -- --ignored --nocapture
cargo test --locked -p p4kx-media --test audio_control_accounting original_atl_source_counts_reconcile_with_assembly -- --ignored --nocapture
```

The sample verifies each selected source checksum, the original spelling and
typed values for all six ATL families, and per-value origins down to the actual
Entry and XML byte or CryXml table position. It serializes values and binds
origins in memory. The whole-root census reads original ATL XML and reconciles
original trigger winners, event order, IDs and localisation with Assembly
AudioIndex. Both installed checks perform zero Media artifact writes.

The synthetic Media replay compares complete exported records and their raw
sidecars. Whole-root Media publication and record-count reconciliation against
an installed Build require separate scale evidence.
