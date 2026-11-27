### Added

- Record exports can retain field-level source evidence for written JSON, NDJSON, XML and flat values. The evidence follows the original record, reference, localization and tag reads where available.
- DataForge queries can write a field-origin receipt for their published rows with `--origin-receipt` or send the evidence to a separate output for stdout queries.
