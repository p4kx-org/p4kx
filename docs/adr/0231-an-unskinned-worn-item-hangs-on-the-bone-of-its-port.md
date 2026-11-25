# An unskinned worn item hangs on the bone of its port

Decided for issue #735. Evidence: the LIVE Archive, `Criminal_Gang_Guard_Heavy_Boss.xml` with the
backpack `doom_combat_medium_backpack_01`.

## What the data says

The core armour `slaver_armor_heavy_core_01_01_01` has the port `backpack`, a `Bone` attachment to
the helper `backpack_attach_1_override` with the item helper `backpack_attach_1_loc` and no
offset. The helper is a bone of the core's skin (below `Spine3`), not of the body skeleton. The
backpack model has no bone weights, so the Armour wrote it as a root node at the origin, between
the feet.

The holder bone and the item locator use authored transforms in the model coordinate system.
In LIVE `13220821`, the DOOM and Palatino Deadlock locators have the same orientation within
float rounding. Each locator's primary and companion matrices agree. The fixed extra 90-degree
X turn makes both rigid shells lie nearly horizontally in the export. Aligning the authored
locator directly with the holder's port places both shells upright. The Deadlock side views
show that orientation in the game. The DOOM export gives the same result at details 0 and 2.

## Decision

- A worn piece that sits on a port with a `Bone`, `Entity` or `StatObj` attachment records the
  port: the helper, the offset, the item helper, and the piece that holds the port, or the body.
  A port with a `Skin` attachment is unchanged (ADR-0224).
- A piece without a skinned mesh at the exported level is placed once the skeleton is complete:
  the helper is looked up among the bones of the holding piece (the body skeleton for the body);
  the piece hangs on the bone of the body skeleton with that name, else on its nearest ancestor
  that the skeleton has, as a rigid child of that bone node in the GLB, so it follows the bone.
- A valid item locator aligns directly with the holder's authored port. The transform below the
  target skeleton bone is the inverse of its bind pose, the holder helper's bind pose, the
  port's authored offset and the inverse of the item locator.
- With no usable item locator, the rigid item uses the 90-degree X turn between the holder helper
  pose and the port offset. Unusable declared locators retain their named diagnostics.
- A helper that is no bone of its holder is listed as `helper_not_found` and the piece stays at the
  origin. A placed piece is no longer listed as `not_skinned`.
- Skinned pieces, and pieces that are not worn armour or clothing such as holstered weapons, are
  not changed; drawing those is the general bone-attachment work of #655.
