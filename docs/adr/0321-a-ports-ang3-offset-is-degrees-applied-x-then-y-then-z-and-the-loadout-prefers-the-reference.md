# A port's Ang3 offset is degrees applied X, then Y, then Z, and a Loadout entry prefers its Reference to its class name

Decided for issue #277. Amends ADR-0040, which left the Ang3 convention unvalidated. Evidence: the
LIVE Archive, `crates/p4kx-assembly/tests/real_archive/real_loadout_semantics.rs` and the unit
tests in `crates/p4kx-assembly/src/placement.rs`.

## Rotation convention

The rotation of a port offset (`SItemPortDef` helper `Offset.Rotation`, an `Ang3`) is in **degrees**
and turns the part about X first, then Y, then Z: `R = Rz · Ry · Rx`, positive angles
counter-clockwise (right-handed), the offset's position added after the rotation. The same holds
for the `Ang3` offset of an object container. `placement::offset` is the only place that applies
it.

The data decides it. Over the 395 spaceship and ground-vehicle Entities of LIVE, 49,035 attached
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

Neither changes a placement yet: they are read so that a front-end can show them and a later change
can keep a gimbal or turret in its own orientation (`no_rotation`) without reading the Record
again. `PortTags` and `RequiredPortTags` were read already; the port container holds no tags of its
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

In LIVE, 538 entries of Entity Records carry both and 432 of them disagree (one more has a
Reference to a Record that is not there). The Reference is therefore the deciding field: the class
name of a disagreeing entry often names the inner part (a weapon) while the Reference points to
the part that holds it (a gimbal). Of 155,900 ports of the 1,144 ships and vehicles that hold an
Entity, 860 in 357 vehicles are filled only by a `defaultItem`. Ambiguous class names take the
first Entity in definition order.

## Not decided here

Applying `no_rotation` to placements, and the Loadouts of XML files, whose entries carry no
Reference.
