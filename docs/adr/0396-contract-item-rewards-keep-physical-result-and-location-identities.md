# Contract item rewards keep physical result and location identities

Decided for issue #1158. Builds on ADR-0268, ADR-0273 and ADR-0357.

## Context

Contract generators can author item and Blueprint results in the same block. Item results have
their own outcome flags, signed quantity and delivery selectors. A location can name a
`MissionProperty` instance elsewhere in the same Generator Record.

## Decision

Keep an ordered item-result projection on each physical Generator definition and link to
that projection from each logical contract entry. Reuse the selected result block for
the parent and sub-contract relationship. Keep all original outcome flags. Use the
canonical arena for structured notification data and join a proven owned location using
the original physical reference steps. Preserve the WeakPointer's Record and instance
variants and the states of omitted, null and unreadable values.

## Consequences

A consumer can join an item's original entity and authored location selector without
deriving either from names, contract titles or array equality. The game decides the
effective delivery destination. Existing currency, Blueprint and reputation projections
keep their contracts.
