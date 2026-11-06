# Catalog v3 and serialization schema

## Decision

The 0.5.0 catalog uses schema version 3. Consumers reject an unsupported version before importing
or deleting entities. Existing fields retain their names, units, ordering and meaning; selection
completeness remains separate from read and projection health.

New alternatives of a tagged payload union are breaking schema extensions. Allowing unknown
object fields does not make a new discriminator value valid against an older closed union.
A new top-level kind such as ResourceType is independently additive and does not by itself require
a version increase.

Generate the catalog schema for serialization, including tagged union payloads. A nullable field
that is emitted as null is required by the schema; a missing field is not the same state. Preserve
open additional fields and the documented extensible scalar value sets. Keep the version constant
shared between the runtime output and schema.

The released v0.4.2 schema is a frozen synthetic contract fixture for compatibility tests. Compare
extensions against that schema instead of regenerating a purported old contract from current types.

## Consequences

Schema tests distinguish unknown additive fields from unsupported tagged variants and distinguish
explicit null from missing payload fields. ResourceType compatibility is tested independently of the
catalog version migration. Documentation and callers use the runtime version constant.
