### Changed

- Selective contracts reuse a completed Generator transaction and publish its validated text
  registrations once. An all-owner query omits the additional-owner search index.
- Marker-mode Catalog field binding avoids encoding contributor identifiers for single-source
  observations that only require an immediate encryption marker.
- Full Catalog production packs idle DataForge contributor records in batches during Loadout
  resolution. The XML reader normalizes only candidate XML paths.
- Catalog production stages Ship and Item Loadout rows in one ordered spool. The sequential writer
  streams each row into compression with bounded buffering. Compact typed Ship facts support the
  placement and resource joins and release duplicate port names and evidence after staging.
