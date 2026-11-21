### Added

- Shared source-use inventories retain the original byte-domain encryption classification as
  required `effectiveEncryption`, alongside each value's direct origin marker. Runtime handles
  and descriptions share that immutable fact; the `encryption` handle getter is an ordinary method.
