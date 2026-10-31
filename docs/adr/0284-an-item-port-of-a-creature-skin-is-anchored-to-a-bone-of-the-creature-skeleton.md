# An item port of a creature skin is anchored to a bone of the creature skeleton

Decided for issue #881, on top of ADR-0229 (an item port of a skinned model is anchored to a
bone) and ADR-0275 (the skins of a creature are written bound to the skeleton of its root).
Evidence: the LIVE Archive, `Kopion_CombatPet`.

Amended by ADR-0297: the skeleton the item ports anchor to is the one of any character root, a
human body rig included.

## What the data says

The collar port of a Kopion (`Collar_ItemPort`) belongs to the skin on `Body_ItemPort` and names
the helper `Collar_Attach_Point`. That helper is a bone of the creature's skeleton, the model of
the root, and no node or bone of the skin's own model. The Assembly looked in the model of the
parent part, the skin, found nothing, listed `helper_not_found` and put the collar at the origin.

## Decision

- **A port's helper is looked up in the parent's model first, as in ADR-0229.** When the parent is
  a skin part (placement `skin`) of a creature whose skeleton file is present, a helper that
  neither a node nor a bone of the skin has is looked up as a bone of the skeleton of the root.
- **The placement is `helper` with the bone's name**; the transform is the bind pose of the bone
  in the space of the skin part, as ADR-0229 composes a bone. The port's offset applies in the
  space of the bone.
- **A helper no skeleton holds** is still listed as `helper_not_found`, and the part sits at the
  parent's origin. A creature without a readable skeleton keeps that behaviour for every port.
- The skeleton is the one of ADR-0275 (`creature_rig`); no second lookup of the creature's
  skeleton exists.

## Consequences

The collar keeps its bind-pose place and does not follow the bone when the skeleton is posed; a
rigid part that rides on a bone needs the mount of the rig form (#926). The tongue of a
Quasigrazer (`Tongue_Attach_Point`) is not a bone of its skeleton and stays a listed note.
