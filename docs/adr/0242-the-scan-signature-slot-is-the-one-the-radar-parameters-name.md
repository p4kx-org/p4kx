# The scan signature slot is the one the radar parameters name

Decided for #830. The physical v3 definition and its source evidence are tracked in #1170.

## Decision

- `definitions.mineableEntities` contains the canonical `EntityClassDefinition` Records under
  `entities/mineable`. Order the rows by textual GUID and physical Record index. Each row retains
  its original mining and radar component positions. An entity with no such components retains
  its header and component-array state.
- The slots of `Components[SSCSignatureSystemParams].radarProperties.baseSignatureParams.signatures`
  correspond to the entries of `RadarSystemGlobalParams.signatureSystemParams.signatureTypeParams`.
  The unique original `displayName` for the scan signature selects an ordinal in that array.
  The Catalog and SDK resolve the ordinal against each source Archive.
- A derived finite value requires matching array lengths, a finite selected slot and at least
  one finite nonzero slot. If a different slot proves the nonzero condition, the selected zero
  is still a value. The derived field carries the signature-array source, the chosen slot,
  the original Locale selector and the evidence for the nonzero condition.
- Missing, ambiguous or unreadable selection yields an absent derived value. The original
  signature array and its per-slot states, exact IEEE 754 bits and finite views remain
  available. An unfiltered Catalog with signature slots reports
  `unknown_scan_signature_slot` when the readable radar data cannot establish the selector.
- Each mining component retains its composition and configuration links. Radar components
  retain their contact type and other authored fields. A provider preset links to entities by
  the authored `entityGuid`. The generic Catalog rules identify development leftovers.

## Consequences

Reordered signature types and corresponding values are interpreted using the new source
ordinal. Consumers can query physical definitions with the SDK and inspect the original
evidence without deriving scanner behavior from the raw radar fields.
