# The model preview lists a model in an Info tab; Export as GLB writes through an observed stream

Decided for issue #109.

## Decision

- The model preview reads every level of detail of the model once and prepares the
  `Scene` of level 0 for the 3D view and a small `ModelInfo` with the counts and notes the Info
  tab lists. The info exists whenever the model could be read, also when the view cannot show it
  (a skeleton file, a model without triangles). A model whose other levels cannot be read falls
  back to level 0 alone, so the view keeps working and the info lists the levels that were read.
- The Info tab is a tab of its own next to Model, laid out like the DataForge summary: the facts as
  key and value rows, then the levels of detail and the chunks that were not read as tables. The
  texts are formatted and laid out once per model, language, width and scale and kept in egui's
  frame data, so a frame allocates nothing for them.
- Hover over the node count, bone count, chunk total or a visible chunk field to see its
  source encryption marker. The preview retains the physical sources and contributing
  uses from the same model read. Each chunk is bound to its admitted model or geometry
  Entry. The origin snapshot is discarded when the Archive changes.
- The material row tooltip reports separate markers for the library name, read status and
  displayed material count or single-material declaration. They use the library selected by
  the drawn model reader. A failed library read retains its attempted use in the inventory.
- The LOD table shows source markers on the vertex and triangle counts of each displayed
  level. The IVO vertex count carries an unknown origin. The material-ID tooltip covers
  the IDs visible in the Info tab and displays that scope when the list is shortened.
- Export as GLB reads the model again for the chosen level only, on its own thread, and writes
  through `ModelSource::write_glb_observed`. The stream reports the bytes written against the length
  the GLB header states and fails with a marker once the caller's `CancelToken` is set; the
  temporary file next to the target is then removed. Cancelling before the first write returns
  `ConversionError::Cancelled` without creating anything.
- The preview reads the model, its geometry companion and every `_lod<n>` companion of the
  Archive, so before reading it checks the size of all of them (`companion_file_names`) against
  `MODEL_FILE_LIMIT` and names the first one above it, as ADR-0031 caps each file.
- The summary of an export lists the chunks of the exported files that were skipped or unknown, the
  materials that fell back to the default material and, with textures, the textures the Archive
  lacks (#105). Names and reasons from the Archive have their control characters replaced.
- One write path, `ModelSource::write_glb_observed`, takes the `AnimationSet` and an optional
  `TextureLoader`; the window passes `ArchiveTextures` over the path index when the materials are
  written with textures, and the set it read for the model when the animations are chosen. The
  other `write_glb*` methods call it.
- The choices (level, materials, texture mip, animations, vertex colours, folder) are kept in the
  settings file as `model_export`; a level the next model lacks is replaced by its most detailed
  one, and a material mode this version does not know reads as plain colours.
- The preview thread counts the animations of the model with `AnimationSet::from_archive` over
  every `.caf` and `.dba` next to it (#107) and the Info tab shows the number an export writes by
  default, so additive animations are not counted; a failed read shows "Not read". The read is
  bounded per file by `MAX_ANIMATION_FILE_LEN` and runs off the UI thread; a header-only count
  waits for #386.
- The dialog offers one choice, all animations or none, and only for a model that has some; none
  is the default, since reading them costs time. The export thread reads the set after the model,
  checks the cancel token again before it writes, and fails with the typed error of the read.
  The summary shows the animations and channels written and lists the additive animations and the
  files that could not be read (`AnimationSet::unread`).

## Considered Options

- Count the chunks per level in the info from a second read of level 0 only: the info would not
  list the other levels, which the dialog needs for its choice.
- Cancel by dropping the thread: the file would be left half written; the cancel token with a
  failing writer removes the temporary file through the existing atomic write.
- Show the info as a panel beside the 3D view: it would take width from a view that is already
  short (D.35); a tab keeps the view as it is.
