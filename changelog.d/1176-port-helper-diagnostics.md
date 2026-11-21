### Changed

- Assembly and worn armour report an unresolved tagged port helper with its original port, helper
  and attached Record when the selector cannot be evaluated. Placement uses the root helper
  when available. A default worn rigid piece with no usable root helper retains the warning
  and stays at the scene origin.
