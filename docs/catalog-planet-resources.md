# Planet resource definitions

`definitions.planetResources` contains one row per physical `.resource` JSON
Entry below `Data/Libs/Planets/resources/`. The selector ignores ASCII case and
slash direction. The normal catalog Filter applies to Entry paths. Rows follow
physical Archive order. `availability` reflects the directory declaration,
including filtered or unreadable Entries. `completeness.planetResources` describes
selection; `exportHealth` describes read and projection outcomes separately.

| Source | Catalog field | Meaning |
| --- | --- | --- |
| Directory | `identity` | Original Entry index, source path and normalized path. |
| `guid` | `guid`, `guidRaw` | UUID in canonical lowercase, and original decoded string spelling. The all-zero UUID has its own diagnostic and remains a present value. |
| `name` | `name` | Original game-source label, including an empty string. |
| `bMinable` | `minable` | Original JSON Boolean. |
| `tintPalettePath` | `tintPalettePath`, `tintPalette` | The authored path and its resolution state. |

Every source-backed field exposes its actual contributor references and immediate
encryption marker. Missing keys, explicit null, empty strings and invalid
values preserve their source distinction. Duplicate JSON members contribute to
an invalid scalar state rather than selecting a winner. Duplicate nonzero GUIDs
within the selected physical definitions retain every row with a field issue.
The per-row `issues` list identifies the field and diagnostic code. Export health
aggregates those represented states. Broken Entry content or integrity prevents
normal publication and leaves readable siblings available in diagnostic mode.

Tint palette paths under `libs/foundry/records/` use the declared DataForge
tint-palette path convention, checking the selected original Record type
`TintPaletteTree`. Each matching candidate retains its physical header evidence.
A target outside the catalog Filter is identified separately from a missing
Record. Archive-relative paths are matched as authored and with a single
`Data/` root where applicable. A resolved Archive target has verified directory
metadata. The target payload is never decoded for this relationship. Unknown,
unreadable, wrong-family and ambiguous outcomes preserve the original path and
available evidence.

The shared reader verifies the Entry checksum and decoded length before
projection. The largest possible allocation is bounded by the platform's
representation and by the actual source length. Every JSON token or open
container consumes bytes in that Entry. The reader checks cancellation during
decoded-byte reads and each traversal event. An unreadable document never
produces a truncated successful row.
