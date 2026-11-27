# Selective resource groups

Use `with_resource_groups` or `with_resource_groups_from` for direct physical definitions and
ordered memberships. Use `with_resource_group_parents` or its `_from` form for the existing
parent projection. `CatalogQuerySession` and `ParsedCatalogQueries` provide both operations on
one Archive/DataForge binding. No complete Catalog, resource reader or world walk is involved.

| Endpoint | Snapshot getter | Unchanged canonical dataset | Source payload scope |
| --- | --- | --- | --- |
| `with_resource_groups` | `definitions()` | `definitions.resourceGroups.rows` | Selected group owners and requested locales |
| `with_resource_groups` | `members()` | `declarations.resourceGroupMembers.rows` | Same selected group owners |
| `with_resource_group_parents` | `rows()` | `projections.resourceGroupParents.rows` | Selected owners and the canonical group-family survey; no locales |

Both endpoints accept `CatalogRecordSelection::All`, `Guid` and `RecordIndex`. All uses ordinary
Catalog family and leftover admission. GUID selection retains every eligible physical alias;
RecordIndex selects one physical owner of this decoded DataForge. Rows are ordered by textual
GUID, then physical index for equal GUIDs. Join related rows by their sourced physical index
within the same Archive/DataForge identity, rather than treating the GUID as unique.

## Direct cells and ordered membership

`ResourceGroupDefinition::header()` exposes the original sourced Record header. `name()` and
`description()` retain public-name and protected game-text references respectively.
`default_thumbnail_path()` and `default_thumbnail_path_svg()` retain the original path cells.
`ResourceGroupMembers::guid()` and `record_index()` identify the physical owner. `groups()` and
`resources()` expose sourced arrays whose individual slots are sourced Record links.

Traverse state before payload. Absent, null, present-empty, unreadable and malformed source
states are distinct. Arrays preserve their authored positions, nulls and repeated entries.
Each present `CatalogRecordReference` exposes its actual target index, original GUID, path,
type and source-resolution status. An actually resolved foreign-family target remains a source
fact; its separate Catalog health policy does not rewrite it into a dangling link. No referenced
resource or unrelated group body is opened merely to expose these original headers.

Definitions and memberships retain the existing canonical cells. Their contract does not contain
a general arbitrary-field SourceArena; this SDK adds none. Blocking unknown or unsupported source
fields remain visible in owner coverage rather than being silently declared handled.

`public_name()` and `game_text()` resolve identities only in the direct snapshot's own text
registries. Requested and available languages stay separate. Names and descriptions can use the
same authored localization key without sharing a text domain. Each language form retains both
the original DataForge key selectors and its actual locale inputs. Its encryption marker covers
all of these contributors; the locale payload's own encryption remains independently readable
in the source inventory.

## Explicit parent projection

The parent endpoint returns a `ResourceGroupParentSnapshot`, which has no text registry and
accepts no language parameter. Its `guid()`/`record_index()` cells identify the selected physical
child. `parent()` is the canonical sourced optional Record link: a present optional None is a
known no-parent result only when its source set is complete.

The parent survey covers all canonically admitted groups in original physical order, including
potential parent owners outside the child GUID/index selection. It does not read Resource or
World bodies. The first group referring to the child's GUID with a different owner GUID wins.
Cycles remain permitted source relationships. Equal GUIDs are excluded as self-parent choices
even when their physical indices differ. Output sorting never changes the selection order.

Existing selected trees are reused and each other needed group body is read once. If there is no
usable selected child owner, no group-family or localization payload is requested. An empty
result still does not prove source-family absence: inspect `availability()` separately.

The parent source cell retains actual original selectors up to the winning slot. Earlier
unreadable possible candidates make its source knowledge incomplete. Later failed candidates
remain dependency coverage observations without contaminating a known earlier winning prefix.
No-parent selection needs all relevant candidates. `coverage()` reports selected owner issues;
`dependency_coverage()` separately describes the demanded canonical group survey. An unknown
header is not a known group or a known absent target merely because its GUID was requested.

## Source context, cancellation and memory

Both results retain their own `source_inventory()`. Resolve direct per-value origins and physical
ancestors in that inventory. `contains_encrypted_source()` can be true, false or unknown;
`source_set_complete()` and actual source-use references remain independent from read/key status.
Attempts and unread targets do not become actual contributors. The two endpoints do not share a
mutable text or origin registry with each other.

Require outer query success, inspect all applicable coverage and handle incomplete/nonpresent
cells before replacing a consumer cache. A callback can receive recoverable partial source
knowledge; its successful return is not publication authorization. Archive-change and
cancellation checks still run after it returns.

The session still owns or borrows the decoded DataForge and its lazy metadata indexes. A direct
point query avoids sibling group bodies. The explicit parent projection retains its necessary
family selector observations and releases unrelated group trees as they are consumed. Existing
sequential original-field preparations are reused; a single body read does not imply constant
memory. Measure the application peak for the requested projection against
the SDK memory budget.

## Installed parity and memory

The Windows AMD64 verifier at `8e85fb65026b660deb1c5d4b82b1b71eff77a0a5`
compared the SDK rows and their contributing source inventory to complete Normal
Catalogs for the same Archive Builds. All three ResourceGroup datasets were
Available. The tables matched all 19 definition, 19 membership and 19 parent rows
on each Build, including original values, physical order and per-value origins.

| Build | Normal producer | Contributing source uses | Physical sources |
| --- | --- | ---: | ---: |
| LIVE `13220821` | `23addc5e164f65a28a6f258302b30453b331edce` | 1,186 | 1 |
| PTU `13260598` | `93d92ca20bef8a8c3731339214a3301124ade7ba` | 1,194 | 1 |

The comparisons checked the complete decoded streams and kept Build and Archive
stamps stable. The higher process lifetime peaks were 1,115,025,408 bytes private
and 1,042,235,392 bytes working set. Four independent cold reads also ran both
routes on both Builds, one route per process without a Catalog frame. Across the
four reads the highest private commit was 1,114,550,272 bytes and the highest
working set was 1,031,684,096 bytes.

Decision: [ADR0409](adr/0409-resource-group-queries-separate-authored-memberships-from-parent-surveys.md).
