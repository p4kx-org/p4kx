# A port's Ang3 offset is degrees with X before Z, and a Loadout entry prefers its Reference to its class name

Decided for issue #277. Amends ADR-0040, which left the Ang3 convention unvalidated. Evidence: the
LIVE Archive, `crates/p4kx-assembly/tests/real_archive/real_loadout_semantics.rs` and the unit
tests in `crates/p4kx-assembly/src/placement.rs`.

## Rotation convention

The rotation of a port offset (`SItemPortDef` helper `Offset.Rotation`, an `Ang3`) is in **degrees**
and turns the part about X first, then Y, then Z: `R = Rz · Ry · Rx`, positive angles
counter-clockwise (right-handed), the offset's position added after the rotation. The same holds
for the `Ang3` offset of an object container. `placement::offset` is the only place that applies
it.

The data decides degrees and X before Z, not the place of Y. The evidence was measured during the
review of PR #371, not reproducible from the repository. Over the 395 spaceship and ground-vehicle Entities of LIVE, 49,035 attached
ports, 2,096 with a rotation and 352 rotated about more than one axis, each placement was
recomputed under 24 conventions (six axis orders, degrees or radians, both signs):

- Degrees, not radians: the most frequent values are 180, +90 and -90, then -20, 60, 45 and 135;
  the largest is 240. Weapons pointing along +-Y: 421 of 439 in degrees against 316 in radians;
  mirrored left and right pairs landing mirrored: 120 of 158 against 105.
- X before Z: on the 352 multi-axis ports the part's right axis stays level (pitch and yaw, no
  roll) for 277 to 280 under the X-before-Z orders against 185 to 188 under the Z-before-X orders.
- XYZ cannot be told from XZY or YXZ by the data, because almost all multi-axis ports have Y = 0.
  `Rz · Ry · Rx` is kept: it is the order the code already used.
- Positive angles fit slightly better than negative ones (weapons off-axis: 18 against 34).

Two unit tests pin it with multi-axis fixtures. A rack at `[90, 90, 180]` points its local Z axis
along +Y (forward) and its twin at `[90, -90, 180]` mirrors it; under Z-before-X the axis would lie
sideways along X. A part at `[30, 0, 90]` keeps its right axis level.

## Port data that is read

`PortDefinition` carries, beside the tags and the flags it already held:

- `no_rotation`: `AttachmentImplementation.constraintParams.noRotation`. In LIVE only the entity
  attachment has `constraintParams` (13,276 ports in 5,516 Records); skin, bone and static-object
  attachments have none, and an entity attachment has no helper offset of its own.
- `detach_direction`: the `detachDirection` vector every port has (68,597 ports in 13,704 Records).

Static placement applies the complete helper transform. A rotated relay fixture
distinguishes that rule from translation-only placement. The stored
`no_rotation` flag remains available to consumers. The entity loadout JSON retains its
authored value and source state alongside the unresolved animation interpretation.
`PortTags` and
`RequiredPortTags` were read already; the port container holds no tags of its
own in LIVE, only `PortFlags`.

## Sub-geometry selection

`select_variant` chooses the variant of an Entity for a list of words: all tags of the variant
among the words, the most distinct tags winning, the first on a tie (ADR-0208). It is a pure
function with unit tests. A paint tag such as `Paint_Base_Red` is a tag like any other.

## Reference and class name

A Loadout entry that names its Entity by `entityClassReference` and by `entityClassName` attaches
the Entity of the **Reference**; the class name is used only when there is no Reference. A port
default (`defaultItem`) fills a port only when no entry names it, and an entry that names no Entity
keeps the port empty.

In LIVE, 539 entries of Entity Records carry both and 432 of them disagree (one of the 539 has a
Reference to a Record that is not there). The Reference is therefore the deciding field: the class
name of a disagreeing entry often names the inner part (a weapon) while the Reference points to
the part that holds it (a gimbal). Of 155,900 ports of the 1,144 ships and vehicles that hold an
Entity, 860 in 357 vehicles are filled only by a `defaultItem`. Ambiguous class names take the
first Entity in definition order.

## Installed standard loadouts

The packaged Windows AMD64 CLI from merge
`8e2ab5df97cf7c08bd6378606ed232930875586e` ran
`entity loadout --stop-at-docked-vehicles --json` on two installed Builds. Each
Archive stamp remained stable. The following counts use the attachment
visibility rules of the Assembly output.

| Build and vehicle | Ports | Attached ports | Visible attachments with authored `no_rotation = true` |
| --- | ---: | ---: | ---: |
| LIVE `13220821`, Hammerhead GS | 459 | 240 | 14 |
| PTU `13284307`, Ursa Medivac | 101 | 45 | 3 |

The JSON carries complete field origins for all 17 selected attachments.
Neither loadout reported an unreadable attachment branch. The two front
airlock control panels on the Hammerhead share an Entity-space door helper.

The authored door action resolves from the Open fragment, through the model's
animation list, to one clip in its animation database. The packaged GLB export
decoded one clip with two mapped channels and no unread animations or
unmapped controllers. The shared helper has no animation channel. Both leaf
nodes have 32 translation keys along the same local axis over 1.667 seconds,
with opposite directions and travel of 0.688 m and 1.415 m. The modeled
opening leaves the panel helper at its bind orientation.

## Runtime interpretation

The meaning of `no_rotation` during animation is unresolved. A discriminating
reference must track an attached part while its helper rotates relative to the
model. The Hammerhead opening only translates its two leaves. XML Loadout
entries have no Reference and use a separate identity rule.
