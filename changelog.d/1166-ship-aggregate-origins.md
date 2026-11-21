### Changed

- Catalog v3: Ship mass, mass basis and hull health carry immediate per-value source origins from
  their actual physical caller, Vehicle XML, Modification and metric fields. Physics fallback uses
  the same calculation as ShipBase. Known absence, unreadable inputs and unavailable original
  coordinates stay distinct. Each caller retains its own source evidence on a cache hit.
