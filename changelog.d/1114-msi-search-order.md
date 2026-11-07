### Fixed

- Windows MSI: order PATH ownership checks around directory resolution and preserve the
  invocation binding through install, repair and rollback. Maintenance noops and disabled
  rollback refusals preserve the existing PATH.
