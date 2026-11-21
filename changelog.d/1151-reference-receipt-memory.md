### Changed

- Share repeated original coordinates between catalog Reference receipts and allocate raw-pointer diagnostic payloads only where present.
- Construct full Reference receipts when the export reports a reference that could not be resolved.
- Store original Record and value coordinates without separate namespace, index and field allocations for each retained Reference operand.
- Share original path operands between sibling references during catalog preparation.
