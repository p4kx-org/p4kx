# Ship base data

`p4kx ships <archive> --out <file>` writes the base data of every ship of an Archive: hull mass,
hull hit points with the destruction and detach thresholds of its parts, cargo capacity, size and
the default Loadout as class names per port path. Every value carries the file and field it was
read from. The decisions are in ADR-0061; the library API is `p4kx_export::build_ship_base` (all
ships) and `p4kx_export::ShipReader` (one ship).

The data is facts only: identifiers, class names and numbers. It holds no name, description or
other game text. Archives of the test Channels carry the notice of ADR-0038 on stderr, as with every
command.

## Command

```powershell
p4kx ships <archive> --out <file> [--format json|ndjson] [--filter <filter>] [--exported-at <seconds>]
```

- `--format json` (default) writes one document, `--format ndjson` one ship per line.
- `--filter` reads only the ships whose record path matches (the rules of `p4kx search`); the JSON
  document then states the filter.
- `--exported-at` (or `SOURCE_DATE_EPOCH`) fixes the export time, so equal input gives equal bytes.
- Exit code 0, or 1 when a ship could not be read: it is left out and listed on stderr with its
  GUID and reason. `--json` prints a one-line summary (shape in [cli-json.md](cli-json.md)).

## Selection and order

A ship is an Entity Record under `entities/spaceships` or `entities/groundvehicles`, the selection
of the catalog's `ships` kind, without the development leftovers (`test`, `template`, `dummy`,
`debug`, `deprecated`, `dev`, and for ships also `temp`, `display`, `hijacked` as whole words),
which are counted as skipped. Version 2 left out the ships with the three ship words, 21 on LIVE
(ADR-0167); version 1 listed them. Ships are ordered by the text of their GUID.

## JSON document

```json
{"schemaVersion": 2, "source": {…}, "filter": null, "ships": [ … ]}
```

`source` is the `source` object of the Basetool catalog (Channel, Build, Archive size and time,
whether the Archive key was there and how many encrypted Entries stayed unread, tool version,
export time). NDJSON has the ship objects only.

## Ship

| Field | Meaning |
| --- | --- |
| `guid`, `className`, `recordPath` | Identity of the ship's Record |
| `implementation` | `file`, `modification`, `patchFile` of the vehicle implementation read; `null` when the ship names none or it cannot be read |
| `vehicleKind` | `spaceship` or `groundVehicle`, see below; `null` when the evidence disagrees |
| `size` | `{value, source}` from `AttachDef.Size`; `null` without one |
| `mass` | hull mass, see below; `null` when neither parts nor physics controller state one |
| `hull` | hull hit points, see below; `null` without a vehicle implementation or when no part states a `damageMax` |
| `cargo` | `{scu, cells, gridsWithIssue, grids}`, see below; `scu` is 0 for a ship without grids and `null` when it has grids and none is readable |
| `loadoutSource` | where the default Loadout is read from: the Record field, or the XML Loadout file (`kind` `loadoutXml`) the Record names |
| `loadout` | one entry per port that holds something |
| `issues` | why data is missing, e.g. a vehicle implementation or patch file that cannot be read |

### Source

`{kind, file, field}`. `kind` is `record` (`file` is a record path), `vehicleXml` (the vehicle
implementation file), `patchXml` (the Modification's patch file) or `modification` (an `Elem` of the
Modification, which sits in the vehicle implementation file; `field` names the Modification).
`loadoutXml` is an XML Loadout file (`file` is its path, `field` is `Loadout/Items`). Sums list the
sources as `sources: [{kind, file, field, values}]`, with the number of values each supplied; only
the origins that supplied a value appear.

### Vehicle kind

`groundVehicle` for a Record under `entities/groundvehicles`, `spaceship` for one under
`entities/spaceships` whose vehicle implementation has no wheel (`class="SubPartWheel"`) or tread
(`class="Tread"`) part. A missing or unreadable vehicle implementation counts as no wheels, so such
a Record under `entities/spaceships` is a `spaceship`. A wheeled or tracked Record under `entities/spaceships` gives `null`: the
folder and the implementation disagree, and the kind is not guessed. The Basetool catalog carries
the value as `vehicleKind` (`basetool-catalog.md`, "Ship base data").

### Mass

The sum of the `mass` of every part of the vehicle implementation after the Modification is applied
(`basis: parts`), in kilograms, rounded to four decimals. Landing gear and Loadout items are not
part of it. When the parts state no mass the ship's rigid physics controller mass is used
(`basis: physics`, a `record` source). Numbers are read as ADR-0050 describes, after dropping
invisible Unicode format characters (U+202C and the like) inside them; `cleaned` counts the numbers
that needed it. A `mass` that holds text but no number is listed in `unreadable` with its raw text
and is not in the sum, never counted as 0. A number followed by anything but white space, `d`,
`f` or `kg` (`12,5x`, `1,5`) is read as its numeric prefix (12, 1) and, when the value enters the
sum, is listed in `partial` as `{part, attribute, raw, read}`. The format characters dropped are all of the
Unicode category Cf (Unicode 16).

### Hull

`health` is the sum of `damageMax` of the parts with a positive one, except parts holding an item
port flagged `uneditable` or `invisible` (thrusters, intakes), `parts` the number summed. The
vehicle implementation states hit points per part and no total, hence the sum over the parts; the
exclusion keeps the replaceable hardware out of the hull. Both rules are read from the game data
and reproduce no reference tool (ADR-0128). `damage` and `health` of a threshold are rounded to
four decimals. `partial` lists the `damageMax` and `detachRatio` values read as their prefix that enter the
hull sum or a threshold; a left-out part, a non-positive `damageMax` or an unused `detachRatio`
is not listed.
`destruction` lists the parts that destroy the ship at `damageRatioMin` (1 when absent) of their
`damageMax` (their first damage behaviour in the group `Destroy`); `detach` lists the parts with a
`detachRatio` and no destroying behaviour, which detach at that share. Each entry has `part`,
`health`, `damage` and the `origin` of the part's `damageMax`. `unreadable` lists `damageMax` and
`detachRatio` values with text but no number, `cleaned` the `damageMax` numbers that needed their
format characters dropped.

### Cargo

A cargo grid is an Entity of the default Loadout (docked vehicles are not entered) whose
`AttachDef.Type` is `CargoGrid`, not a template (class name ending in `_template`, or a record
path ending in `_cargogrid_template`, both matched ignoring case). Its capacity
is read from the `InventoryContainer` Record its `SCItemInventoryContainerComponentParams` name in
`containerParams`:

- an open container (`InventoryOpenContainerType`) from its `interiorDimensions` in metres:
  `scu` is x · y · z / 1.953125 (1.25 m cubed), `cells` the product of `floor(side / 1.25)` per
  side;
- a closed container (`InventoryClosedContainerType`) from its `capacity`:
  `SStandardCargoUnit` as it is, `SCentiCargoUnit` divided by 100, `SMicroCargoUnit` divided by
  1,000,000; it has no cells.

Each grid lists `port` path, `className`, `guid`, `container`, `shape` (`open` or `closed`), `scu`,
`cells`, `dimensions`, its `source` (the container Record and field) and an `issue` when its
capacity cannot be read. The ship's `scu` and `cells` sum the readable grids, rounded to four
decimals; `gridsWithIssue` counts the others, and when it is not 0 the sums are a lower bound. A
container with a side that is not a positive length up to 1000 m, or a capacity outside 0 to 10^9
SCU, is not readable (an `issue`); sums saturate and never overflow.
On LIVE, 76 grids carry an `issue`, in two kinds that are facts of the game data, not of the
reading: the Javelin's and the Hornet F7C's grids refer to `InventoryContainer` Records that do not
exist (dangling References, so no capacity), and `MISC_Hull_A_CargoPlate` is a `CargoGrid` item
without an inventory container component. Both are listed with their reason and add nothing to the
sum. About half of the ships have no grid in their Loadout and so `scu` 0.

Cargo that the Loadout does not reach (other containers, interior storage) is not counted.

### Loadout

`loadout` lists every port of the default Loadout that holds an Entity or an attachment that cannot
be resolved, depth first in port order: `port` (names joined by `/`), `className`, `guid`,
`origin` (`loadout`, `override` or `port_default`, ADR-0012), `suppliedBy` (a source: the Record
or XML Loadout file whose entry supplied the attachment, the Record that defines the port for
`port_default` (field `Components/SItemPortContainerComponentParams/Ports`), an included XML file as its own file), `unresolved` (the reason, or
`null`) and `dockedVehicle` (present and `true` for a docked vehicle: its `AttachDef.Type` starts
with `NOITEM_Vehicle`, ignoring case; it stays in its port and its own ports are not listed or counted). The walk
is `EntityNode::loadout` of `p4kx-assembly`, which the Basetool catalog's loadout kind shares.
