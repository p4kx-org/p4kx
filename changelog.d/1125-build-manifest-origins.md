### Added

- Media exports retain field origins for launcher Build declarations in Archive-root records
  and the Index, with original manifest byte counts and verified projection receipts.
  Missing or unreadable manifests remain unknown; malformed completed reads retain their
  actual source without inventing a stored-null value.
