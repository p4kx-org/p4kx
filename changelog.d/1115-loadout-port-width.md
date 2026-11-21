### Changed

- Resolve every admitted Loadout port in authored order. The Rust API removes
  `MAX_LOADOUT_PORTS` and `LoadoutError::TooManyPorts`, and Loadout results no longer emit
  `too_many_ports`.
