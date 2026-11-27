### Fixed

- BuildingBlocks screens preserve clipping beyond 24 logical masks, including nested boundaries
  and SVG output. Rasterization continues to admit actual live mask bytes and drawing work.
