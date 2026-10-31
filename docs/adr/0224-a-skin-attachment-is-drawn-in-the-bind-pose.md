# A skin attachment is drawn in the bind pose

Decided for issue #771. Evidence: the LIVE Archive,
`NPC_Archetypes-Male-Human-MissionGivers-MilesEckhart`.

Amended by ADR-0297: a rigid prop that is no character item hangs on the bone its skin port names.

## What the data says

The body `m_body_01` holds clothing on ports whose attachment is an
`AttachmentImplementationSkin`: `Clothing_Legs` names the helper `Hips`, a bone, and an `Offset`
of x = 0.05. A skin is skinned to the skeleton and follows it by its weights, so the clothing is
already in the body's bind pose. The Assembly looked the helper up as a node, found none, listed
`helper_not_found`, placed the pants at the origin and then added the offset, so they sat 5 cm
off to the side. Moving them back by 0.05 centred them on the body.

## Decision

- A part on a port whose attachment is `Skin` is placed at the identity in the parent's frame:
  neither the helper nor the offset moves it, and `helper_not_found` is never listed for it.
- The attachment type decides, not the helper: a `Skin` port without a helper node is a skin too.
- Its placement is the new `Placement::Skin`, code `skin`, so a package tells a skin from a part
  that sits at the origin because it found no helper.
- Every other attachment (`Bone`, `Entity`, `StatObj`) is placed as before.
