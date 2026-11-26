### Fixed

- Catalog file export checks the Archive again before publication and preserves the previous
  file if the input changes. Catalog input decoding observes cancellation.
- The CLI reports recognized ship and DataForge input changes with exit code 4. Schema commands
  run without Archive setup and honor compact JSON output.
