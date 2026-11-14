# The bone attachments of a character definition are models the part carries at its bones

Decided for issue #655. It completes the third section of ADR-0150, which drew the skin of a
character definition and left a second model per part undecided. Evidence: the LIVE Archive, the
main thruster of the M50 (`ORIG_M50_Main_Thruster_Nozzle.cdf`) and of the Gladiator.

## Decision

- **Reading.** An `Attachment` of type `CA_BONE` with a `BoneName` and a `Binding` is a bone
  attachment unless it is the drawn model. `RelPosition` (`x,y,z`, metres) and `RelRotation`
  (a quaternion `w,x,y,z`, `1,0,0,0` being none) default to no offset; a value that does not read as
  numbers, a rotation without length or a binding that is another definition is listed as left
  out. Every readable bone attachment is retained in source order; `left_out` retains every
  original undrawn occurrence without a count prefix. The numbers are read with the tolerant
  parser of ADR-0145. Source Entry admission and per-definition vertex admission are independent.
- **Providing.** `ProvidedModel::attachments` holds one `ProvidedAttachment` per bone attachment
  that was read: its name, bone, offset, rotation, entry and model. A file that is missing or
  cannot be read is listed in `left_out`, so `attachments_left_out` names only what is not drawn.
- **Placing.** A bone attachment is not an Entity, so it is no part. A `Part` carries
  `attachments()`, each an `AttachedModel` with its own model and material library (the attachment's
  `Material` replaces the one the model names). Its transform to the part is the bind pose of the
  bone in the part's model, `bone_transform` as for the backpack and the item ports of a character
  definition, then the attachment's own offset. A bone the model lacks is `helper_not_found`, and
  the model sits at the part's origin with its offset.
- **Writing.** The GLB, the Blender package and the viewer scene write each attachment as a child
  of its part's node, after all parts and before the Interior, in the order of
  `Assembly::attached_models`; the indices of the parts are their own. A viewer piece has the path
  `<port path>/<name>`, so the tree lists it.
- **Limits.** The numbers of an attachment must be finite as `f32`, else it is left out. The
  models of one definition, skin and attachments together, are read up to
  `ArchiveModels::with_max_attachment_vertices` (by default `MAX_ASSEMBLY_VERTICES`); an
  attachment that passes it and the ones after it are left out.
- **Budget.** The vertices of the attachments count with those of the part's skin against the
  vertex budget, in the wave check and the part-by-part check, and against the bound of the model
  cache.

- **Storage.** Within one character read at one LOD, successful repeated attachment models share immutable storage by the actually selected canonical Entry and the exact parsed material override. Each instance still selects its current Entry first and keeps its own name, bone, transform and vertex charge. Missing, unreadable and fatal outcomes keep their existing handling; the local cache is discarded when that character read ends.

## Consequences

The thruster housings of the M50 and the Gladiator are drawn below their nozzle and are not in
`attachments_left_out`. The tint palette of the part applies to its
attachments. A landing gear's attachments are placed at the bind pose of its skeleton, as the
bone is looked up in the model the part draws.
