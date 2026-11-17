### Added

- Render the default state of Secondary screen buttons from the authored proxy and ordered template
  parts. The instance supplies its label, visibility and positioning. Shared brand styles can supply
  the frame SVG and its scaling and nine-slice settings.
- Render the default frame and label of General screen buttons through their standard template, preserving authored label visibility.
- Render GeneralLarge buttons from their original scene root and ordered frame, icon and label
  parts. Empty automatic leaves take zero space before margins during flex measurement, so
  their containers leave room for neighbouring buttons.
