# An item port of a skinned model is anchored to a bone

Decided for issue #749. Evidence: the LIVE Archive, `MISC_Prospector`.

## What the data says

The mining arm `MISC_Prospector_Mining_Arm` draws the character definition
`misc_prospector_drill_arm.cdf`, whose model is a skin with a skeleton. Its port
`hardpoint_mining_laser` holds the gimbal `Mount_Gimbal_Mining` at the helper
`BONE_ItemPort_DrillBit`. That helper is a bone of the skeleton, not a node of the model. The
Assembly looked it up as a node only, listed `helper_not_found` and put the gimbal, and the laser
below it, at the arm's origin, inside the nose.

## Decision

- A port's helper is looked up as a node of the parent's model first, as before; when the model
  has no such node, as a bone of its skeleton, ignoring case.
- A bone resolves to its bind pose in the model's space, composed from the poses relative to its
  parents. The port's offset applies in the space of that bone, as for a node.
- Only the bones of the first level of detail count; of several bones with the same name, the
  first wins.
- A node and a bone of the same name resolve to the node.
- The placement is `helper` with the bone's name.
- A helper that is neither a node nor a bone is still listed as `helper_not_found`, and the part
  sits at the parent's origin.
