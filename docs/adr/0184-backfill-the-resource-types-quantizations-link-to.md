# Backfill the resource types that quality quantizations link to

Decided for issue #699. Builds on ADR-0177.

## Decision

- **A `ResourceType` that a kept quality quantization links to is listed as a commodity**, as one
  that a blueprint cost uses already is, unless a commodity Record carries it. Every
  `resourceTypeGuids` entry then resolves to the `subtypeGuid` of a catalog entry (25 more
  distinct `ResourceType`s on LIVE).
- **The entry is built exactly as the existing backfill builds it**, with no new field. Like
  every commodity it carries `name` (a short name) and `description` (a game text,
  `x-gameText`), and it falls under the game-text policy of `docs/catalog.md`: the
  catalog keeps the `description` for the Basetool's logged-in members, the Basetool keeps it
  away from third parties.
- **Leftovers stay out:** a development leftover `ResourceType` neither links (ADR-0177) nor is
  backfilled by a quantization. With a `--filter`, an entry is listed only when its record path
  matches.
- **Part of the unreleased `schemaVersion` 2**, with no further bump.

## Consequences

- The `commodities` kind of LIVE grows by the quantized `ResourceType`s no blueprint uses and no
  commodity Record carries.
- ADR-0177's statement that only 18 of the 43 links resolve is superseded.
