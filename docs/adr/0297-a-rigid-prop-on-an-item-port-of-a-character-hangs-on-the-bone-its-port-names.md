# A rigid prop on an item port of a character hangs on the bone its port names

Decided for issue #890, on top of ADR-0224 (a skin attachment is drawn in the bind pose),
ADR-0229 (an item port of a skinned model is anchored to a bone) and ADR-0284 (an item port of a
creature skin is anchored to a bone of the creature skeleton). Evidence: the LIVE Archive,
`NPC_Archetypes-Male-Human-MissionGivers-MilesEckhart`.

## What the data says

The root of a human NPC names the body skeleton (`bhm_skeleton_v7.chr`, 220 bones). Its body is a
skin on a skin port, and the item ports of that skin hold two props:

- The mobiGlas sits on `mobiglas_attach`, whose attachment is `Skin` with the helper
  `mobiglas_attach`. That is a bone of the skeleton and none of the body skin. The model
  (`mobiglas_civilian_prop_01.cgf`) has no mesh weights: its vertices lie within 4 cm of its own
  origin. ADR-0224 drew it at the identity, so it sat between the feet with its screen on the floor.
- The tumbler sits on `weapon_attach_hand_right`, a `Bone` attachment to `RightWeaponBone`, again
  a bone of the skeleton and none of the body skin. ADR-0284 anchors such a helper only for a
  creature root, so it was listed as `helper_not_found` and drawn at the origin.

Clothing, hats and shoes whose own geometry is a world prop (a cardboard box) sit on skin ports as
well; they are character items (`Char_*`) and the box is not meant to be worn.

## Decision

- **The skeleton of the ports is the root's model when its geometry is any character file below
  `Objects/Characters`**, the human body rigs included. The skins of a human root stay unbound, as
  in ADR-0275; only the lookup of an item port's helper uses the bones.
- **A helper that neither a node nor a bone of the parent's model has is looked up as a bone of
  that skeleton** for every port below a skin part, as ADR-0284 decides for creatures. The
  placement is `helper` with the bone's name and the transform is the bind pose of the bone in the
  space of the skin part. The port's offset applies in the space of the bone.
- **A prop on a skin port hangs on its bone too.** A prop is an Entity whose item type is no
  `Char_` item and whose model has no skinned mesh (a model without meshes counts). Such a part has no weights
  that could follow the skeleton, so the attachment type `Skin` does not put it in the bind pose.
  Its helper is looked up in the skeleton only, not in the parent's model, so a rigid part whose
  helper is a node of the skin keeps ADR-0224.
- **Anything else on a skin port keeps ADR-0224:** skinned parts, character items, and a prop whose
  helper is no bone of the skeleton, without a note.
- **No turn is applied.** The bone's frame carries the prop as it is: in the bind pose the tumbler
  lies in the palm of the right hand and the mobiGlas sits flat on the left forearm, checked in
  Blender. The turn of ADR-0231 belongs to the item helpers of worn armour.

## Consequences

The mobiGlas, its screen and the tumbler sit at the body bones in the bind pose; the screen takes
its place from the mobiGlas, as its own port names a node of that model. The weapons NPCs hold on
`RightWeaponBone` (no longer `helper_not_found`) hang on the right hand in the bind pose as well;
their orientation follows the bone's frame and is not checked against a grip. A part in the bind
pose does not follow the bone when the skeleton is posed; that needs the mount of the rig form
(#926).
