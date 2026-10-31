# The alternative of a generated Loadout group is picked by index

Decided for issue #954, on top of ADR-0289 (a generated Loadout takes the first alternative of
each group of its assortments) and ADR-0302 (which left the pick to this issue). The owner decided
the product questions on the issue on 2026-10-31.

## Decision

- **An index, never a seed.** The game picks one alternative per group at random and the Archive
  holds neither the generator nor its seed, so a seed could not reproduce the game's pick; it
  would only be an invented mapping. An index is deterministic and can be checked against the
  group.
- **Per group, by name.** A pick is `<group>=<index>`, e.g. `Body=2`. A group that is not named
  keeps its first alternative, so without picks nothing changes. A bare `<index>` takes that
  alternative in every group that has it; a group that lacks it keeps its first alternative, and
  that is no issue. A named pick wins over the bare one. A bare index beyond every group is no
  issue either, on purpose: it means "every group that has it", so none having it is the defined
  result, and the bare form names no group that could be misspelt. A bare typo such as `9` for
  `0` therefore stays silent; a named pick is the form that is checked.
- **Indices count from 0** (owner decision, confirmed), as `--lod` and `--texture-mip` do: 0 is
  the first alternative. An index is ASCII digits only and must fit `usize`; a sign (`+1`, `-1`)
  or a value that overflows is no pick.
- **The key is the group name alone**, matched ignoring ASCII case, in every assortment that has
  a group of that name. The assortments of one Loadout share a handful of group names (`Body`,
  `TorsoAttachments`), and an export asks for "the body", not for one assortment's body. An
  assortment GUID in the key would need the user to look GUIDs up and would not carry from one
  Entity to another.
- **A named pick of a group that no group of the whole Loadout has is an issue** (owner decision).
  The resolver collects the names of the groups of every assortment it meets while resolving the
  Entity, with the Entities attached below it; a named pick whose name is not among them is one
  `assortment` issue of the root Entity that names the pick (`Bodyy=2`), so a typo does not pass
  silently. The export still succeeds and nothing else changes.
- **An index beyond a named group is a Loadout issue** (`assortment`, the assortment's GUID and
  the group in the reason) and the group keeps its first alternative, so the export is not empty.
  An alternative that is no Loadout is the same issue as for the first alternative (ADR-0289):
  the group gives nothing, and the others are not tried.
- **The picks are part of `LoadoutChanges`.** Every front-end that applies changes
  (`entity loadout`, `entity glb`, `entity package`, the GUI's Loadout and export) then carries
  them without a second path, and a replacing Entity's own generated Loadout follows the same
  picks. `LoadoutChanges::is_empty` counts them; whether a port plan is needed does not.
- **The cache holds every alternative, the pick is applied after the read.** `Assorted` keeps
  each group with all its alternatives, so one read of an assortment serves every pick and the
  pick is not part of the cache key. A resolver takes the picks of the changes it resolves; the
  plain `resolve` resets them.
- **Surface.** `--loadout-pick <group>=<index>` on `entity loadout`, `entity glb`,
  `entity package` and `ui hologram`, which share the Loadout options; the `loadout_pick`
  parameter of the MCP tool `entity_loadout` (a list of the same texts; the tool's tree cache and
  its cursor take the picks into their key, normalised: group names in lower case and sorted); a
  field of the GUI's Entity export that shows the picks of the Loadout it opens with and blocks
  the export while its text is no picks. Several picks are separated by commas everywhere, in the
  CLI also by repeating the option. A text that is no pick exits the CLI with code 2, before the
  Archive is opened, and is a tool error in the MCP server; a Loadout that cannot be resolved
  stays the Loadout error it was. The picks are not written into the Blender package's
  `scene.json`; the package holds the resolved scene.

## Consequences

An export can show any alternative of a creature's or an NPC's generated Loadout, and the group
names to use are those `p4kx entity loadout` lists in the reasons of the assortment issues and
those of the assortment Records. The GUI lists no groups to choose from; a list needs the
assortments of the selected Entity read ahead, which is a follow-up.
