# A creature with its skeleton is packaged as one skinned rig mesh

Decided for issue #926, on top of ADR-0275 (the skins of a creature are written bound to the
skeleton of its root), ADR-0107 (a Character is one rigged GLB mesh behind a `character` node) and
ADR-0307 (`entity package` stays static). Evidence: the LIVE Archive, `Kopion_CombatPet`, and the
creature fixtures of `p4kx-testkit`.

## What the data says

`entity package` wrote every distinct mesh of the Assembly as its own static GLB, which the add-on
links to as many objects as nodes draw it. A creature whose root geometry holds a skeleton
(`Assembly::creature_skins` is `Some`) is skinned in `entity glb`: the bones are nodes, one glTF
skin spans them, the skins are bound to it and the parts on its bones and item ports hang below
the nodes they follow (ADR-0275, ADR-0297). In a package the same creature came out as a set of
static meshes with no bones and no weights.

The Character package already has the form a skinned model needs: one GLB with the bones, the
skin and the animations that the reader imports once, not as an instance, and a mesh `rig` that
describes it. The add-on imports every rigged mesh that way whatever the kind of the node that
draws it.

`Kopion_CombatPet` (LIVE, build 4.10.197.2878, change 12913470) written this way holds 77 bones,
one skinned mesh, a collar and a horn on item ports and 5 materials in one GLB of 120 MB at mip 0, and imports in Blender 5.2 as one armature with 77 vertex groups
on the skinned mesh and the collar and horn parented below.

## Decision

- **A creature with its skeleton is one rig mesh.** `write_package` writes `meshes/m0001.glb` as
  `Assembly::write_glb_with` does, with every part of the Assembly in it, and one node of the kind
  `entity` (the root, identity transform, its `entity` reference) drawing it. The mesh's `rig`
  lists the file's bones, skinned meshes, morph targets and the skinned parts. The `issues` are
  those of the Assembly. A creature without a skeleton file, a skeleton without bones, or without a skinned
  part (the writer binds no bones then) and every other Entity keep the static meshes. The
  Interior of such a creature is not written; `--interior` gives the issue
  `interior_not_written` and no Interior nodes.
- **The materials are in the file.** As in the Character package the textures are embedded at
  `texture_mip` and the mesh lists no `materials`; `textures`, `materials` and `palettes` are
  empty. `--texture-mip` keeps its default 0, so a creature with big textures gives a large file
  (about 110 MB for the Kopion); a higher mip makes it smaller.
- **The node is an `entity`, not a `character`.** A creature is an Entity with a GUID, and the
  `character` kind says that the package was made from a CHF or a body. The add-on needs no new
  rule: it already imports the mesh of any node whose mesh has a `rig` once.
- **The rig says what a creature lacks (format 1.8).** `rig.body` is optional and absent for a
  creature; `rig.skeleton` is the geometry file of the root, whose model holds the bones (the
  Assembly keeps no separate skeleton path: a character definition names its skeleton inside
  itself, and the root geometry is the one file both forms share); the part
  origin `skin` names a skin of a creature. Both are additive for a reader that ignores unknown
  values, but a reader of 1.7 requires a `body`, so the minor version rises to 1.8 and the add-on
  reads it (`SUPPORTED_MINOR` 8).
- **No animations.** The file has none and `rig.animations` is empty, as ADR-0307 decided for
  `entity package`.
- **One writer for both rig packages.** `rig_package` writes the folder, the file, the manifest
  and the summary of a package with one rigged mesh; the Character and the creature give it the
  GLB and the manifest content.

## Consequences

- The shape of a package changes for creatures with skeletons: one mesh and one node instead of
  one mesh per part and a node per part. A reader that follows the manifest gets the parts from
  the GLB, whose node names are those of the parts.
- The textures the file could not embed are `texture` issues in the manifest, for the Character
  package too.
- The report of `entity package` prints the bones and skinned meshes and its JSON has `bones` and
  `skinned_meshes` for such a creature; both are absent otherwise.
- A creature written twice with different loadouts gives two packages that do not share meshes.
  Sharing across Entities was never a goal of a package of one Entity.
