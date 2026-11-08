# Catalog v3 and serialization schema

## Decision

The 0.5.0 catalog uses schema version 3. Consumers reject an unsupported version before importing
or deleting entities. [ADR-0356](0356-catalog-v3-uses-canonical-source-typed-tables.md) defines the
canonical target structure and explicitly retires redundant paths from the unpublished draft,
including items[].stats and duplicated key/en/de views. Preserve source meanings, raw units,
authored ordering, selection and identity algorithms through the documented migration. Selection
completeness remains separate from read and projection health. Per-data-value source encryption
origin is mandatory under [ADR-0357](0357-source-encryption-provenance-is-explicit-per-data-value.md).

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

Version 3 also carries measured `exportHealth` separately from Filter completeness (ADR-0347).
Normal publication requires healthy source reads and projections; explicit diagnostic publication
retains incomplete status, exact affected source identities and observed counter units in the
artifact. Consumers require both healthy export and unfiltered selection before inferring removals.

## Scalar vocabularies and fixed controls

The extensible scalar rule of ADR-0054 applies to source vocabularies, identifiers and provenance
annotations. Their schemas remain strings with documented `x-values` and
`x-p4kx-vocabularyClass: "open"`. Unknown source labels never imply a known priority, resolved
target or successful operation. Kind identifiers, Entry-operation identifiers, diagnostic codes,
negative-only unresolved-reference reasons and explanatory Archive-key states remain open.

Schema 3 distinguishes eleven fixed control types: `CatalogPublicationMode`, `ExportHealthStatus`,
`ExportStageStatus`, `ExportSourceMode`, `ExportDiagnosticSeverity`, `ExportDiagnosticStage`,
`ExportDiagnosticUnit`, `ExportSourceRole`, `DefinitionStatus`, `OwnedReferenceStatus` and
`PropertyReferenceStatus`. They govern publication, health accounting or a positive source-link
result. Their finite serialized string alternatives remain closed and carry
`x-p4kx-vocabularyClass: "control"`; an arbitrary string is invalid. A new control alternative
requires contract review and a version decision if it changes the consumer's interpretation.

This deliberately qualifies the scalar rule for the unpublished version-3 contract. It changes
schema acceptance, not any existing runtime enum value, precedence, selection or counter unit.
Control closure does not establish source truth: physical target identity, owner-scoped links,
measured health, Filter completeness and the normal-publication gate retain their independent
checks. The released version-2 fixture remains unchanged.
