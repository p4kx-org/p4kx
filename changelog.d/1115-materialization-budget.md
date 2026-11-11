### Added

- DataForge library callers can set an explicit per-read allowance for canonical Value
  materialization. Exhaustion refuses the read before output reservation, while ordinary
  tree builds remain without a fixed Value-count quota.
