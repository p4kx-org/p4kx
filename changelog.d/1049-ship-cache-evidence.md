### Fixed

- Catalog: shared ship loadout files retain their original read and parse evidence across
  successive cached ship reads without replaying file-operation rows or diagnostics.
  Concurrent read and parse callbacks retain their exact original document-operation identity.
