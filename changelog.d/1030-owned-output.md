### Fixed

- Archive-derived writes retain source protection through temporary files, final publication and
  cleanup. Descriptor-bound ownership permits fresh outputs without treating unverified existing
  portal targets as safe. Cancellation preserves independent cleanup and retained-output failures.
- Directory namespaces authenticate their own root directly and recheck it during cleanup,
  without treating virtual Documents-portal parents as writable output directories.
- Catalog and single-file Record exports explicitly clean failed unpublished output and keep
  prior output intact; unpublished Record rows are not reported as written.
- Blender package placement, backup and restoration preserve occupied foreign targets and retain
  both errors if recovery fails. Media generations reuse the persistent empty lock without
  changing its contents or deleting its inode while other exporters may be waiting.
