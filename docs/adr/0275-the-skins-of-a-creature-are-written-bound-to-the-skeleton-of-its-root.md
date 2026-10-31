# The skins of a creature are written bound to the skeleton of its root

Decided for issue #880, on top of ADR-0224 (a skin attachment is drawn in the bind pose),
ADR-0229 (an item port of a skinned model is anchored to a bone) and ADR-0255 (the skeleton of a
character definition is checked). Evidence: the LIVE Archive, `Kopion_CombatPet`.

## What the data says

The root of a creature Entity names a skeleton file (`kopion_skel.chr`, no mesh) or a character
definition. Its body sits on a skin port, and the skin (`m_body_kopion`) and its collar are
skins weighted to the bones of that skeleton. ADR-0255 drew them static: the bind pose as plain
meshes, no joints, so a creature could not be posed in Blender or a viewer.

The census rule of ADR-0255 (a `.cdf`, `.chr` or `.skin` root outside `Objects/Characters/Human`)
also matches animated parts. Folders of the LIVE roots that are skinned with a path-only gate
(skinned / static): `buildingsets/human` 78 / 4, `characters/banu` 5 / 0, `characters/creatures`
102 / 12, `characters/mobiglas` 1 / 0, `characters/powersuit` 9 / 0, `characters/vanduul` 12 / 3,
`fps_weapons/*` 424 / 48, `planets/flora` 1 / 0, `props/flair` 3 / 0, `spaceships/ships` 35 / 4,
`spaceships/thrusters` 33 / 0. The non-character ones are weapons, gadgets, doors, thrusters and
ships whose moving parts carry a skeleton; skinning them gains nothing and changes files that
had no reason to change.

## Decision

- **A creature is an Entity whose root geometry is a character file below
  `Objects/Characters` outside `Objects/Characters/Human`** (`is_creature_path`): the census
  rule narrowed to the folder of the character models. In LIVE that is `banu`, `creatures`,
  `mobiglas`, `powersuit` and `vanduul`. The real-Archive test asserts that the skinned Entities
  lie in those folders and that the creatures are among them.
- **The skeleton is the bones of the root part's model**, at the exported level of detail. A
  `.chr` root is the skeleton file; a `.cdf` or `.skin` root is a skin whose own bones are the
  skeleton.
- **`Assembly::write_glb` binds the skinned meshes to it.** The scene writer takes the skeleton
  (`Scene::with_skeleton`) and does what the armour writer does: the bones once as nodes, one
  glTF skin with the inverse bind matrices, `JOINTS_n` and `WEIGHTS_n` on every skinned mesh with
  its bones matched to the skeleton by name (`BoneMatch`). The node of a skinned mesh is a root
  without transform, the nodes below it keep their place (ADR-0039). Unskinned parts stay where
  the Assembly placed them. No second skinning path exists: the scene writer reuses the
  remapping and the inverse bind matrices of the rigged writer.
- **The root is at the identity.** The skeleton is written in the Assembly's space, so the root
  part and, per ADR-0224, every skin part sit at the identity. glTF ignores the transform of a
  skinned mesh's node, so the port offset of a skin part has no effect.
- **Bones the skeleton lacks** follow their nearest parent bone it has, and the part gets the
  note `bones_not_in_skeleton`, the code of the armour path, with the count.
- **A missing or unreadable skeleton file** (`skeleton_missing`, `skeleton_unreadable` on the
  root) keeps the creature static, as before. A scene without a skinned mesh adds no bones.
- **The JSON report of `entity glb`** counts `bones` and `skinned_meshes`.
- **Animations are not written.** (Superseded by ADR-0307.) An Entity names no animation: its
  clips are chosen through the animation database of the creature, which the Assembly does not
  read. #925 records that.
- **`entity package` stays static.** Its meshes are separate files that the add-on instances;
  a skinned creature needs the rig form of the Character package (#926).

## Consequences

Rigid parts on a bone of the skeleton, such as a horn, keep their bind-pose place and do not
follow the bone (#881 anchors item ports to bones). The weapons, gadgets and ships of the census
stay as they were.
