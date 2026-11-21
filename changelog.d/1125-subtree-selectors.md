### Fixed

- Distinguish DataForge subtree selection from field payloads. Inline classes retain their
  layout and array selectors, and Strong Pointer words retain their own selector coordinate.
- Include the actual container selection path for descendant fields in complete observed Records.
- Use the original Reference GUID, pointer selection and resolved target facts for each emitted member in Record JSON and comparison.
