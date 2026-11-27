# The model preview counts animations after the model, within a bound

Decided for issue #415. Follows up the Info tab and Export as GLB of ADR-0047.

The preview read every `.caf` and `.dba` file next to a model before it sent the scene, without a
bound and without looking at a stop flag. A folder of thousands of animation files, or a model with
nothing to animate, held back the 3D view of any model in it. The Info count also counted
animations the export could not write.

## Decision

- **Scene first, count second.** The preview thread sends the scene and the facts of the model as
  soon as they exist, with the animation count `Reading`, and the count and the animations that
  move the skeleton in a second event. `ModelPreview::is_reading_animations` tells which state it
  is in. The Export as GLB is unavailable until the count is known.
- Hover over the animation count for its source encryption marker. The same bounded
  read retains one explicit use per distinct Archive Entry containing a selected
  animation. Additive and unread clips fall outside this count. The marker records
  incomplete source coverage. A count without a source witness has unknown origin.
- **A bound.** `ArchiveAnimationRead::for_model_bounded` reads the animation files of the model's
  folders unless they are more than `AnimationBounds::files` files or `AnimationBounds::bytes`
  uncompressed bytes, checked before any is read. The preview passes 64 files and 64 MiB
  (`PREVIEW_ANIMATION_BOUNDS`). Beyond the bound the count is `AnimationCount::TooMany`, shown as
  "Not read"; the export setup still offers the animations with a warning that they are read in
  full and the file can be large, and the export reads them without a bound but stops between
  files when it is cancelled (`AnimationSet::from_archive_cancellable`).
- **Stoppable.** The read takes the preview's `CancelToken` and stops between files with
  `ConversionError::Cancelled`; dropping the preview cancels it. #386 (a header-only count) stays
  the real fix for the cost per file.
- **Nothing to animate, nothing read.** A model with no bones, or with bones that neither belong
  to a written skeleton nor drive a node, as `p4kx_gltf` targets them, counts `Counted(0)`
  without reading a file. Whether the model is skinned comes from the model (a bone map in a mesh
  of any level of detail), never from the viewer's scene, which can reject a model the export
  writes.
- **Written is not found.** The export summary lists an animation that was read but has no
  channel, because no track matches a bone of the model, under "Animations not written", with the
  number of tracks left out (`AnimationNoteKind::NoBoneMatched`). The list of not written
  animations has its own cap, `MAX_LISTED_ANIMATIONS`.

## Consequences

- A test waits for `is_reading_animations` to end before it reads the count.
- The cancel path between reading the model and writing the file is a function of its own in the
  export job, with a `test-hooks` pause at the seam, so a test cancels after the read without
  depending on timing.
