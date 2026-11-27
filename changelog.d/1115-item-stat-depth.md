### Fixed

- Catalog: item-stat lookups retain readable nested fields and types beyond the former
  16/32-level cutoffs, preserving root precedence and stored depth-first order with an
  iterative walk over all materialized owned values.
