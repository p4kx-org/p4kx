# Mission location values retain original source cells

Unpublished Catalog schema 3 exposes every Mission Location datum through the shared source
carriers, as required by ADR-0356 and ADR-0357. The existing dataset address and selection remain
unchanged. Its flattened CanonicalRecordHeader replaces raw identity strings, Bool fields retain
their individual source states, entityClusterMember retains the original Reference and resolution
state, and names retains both the original collection state and every physical member slot.

The original first-readable-Struct policy remains authoritative for locationData and
stringVariants. OriginalFlowFields and the existing location Reader retain the exact prefix
selection, including an unreadable predecessor; no strict scalar-Class reinterpretation or
later DTO equality can replace that observation. Each dependent immediate cell retains those
actual selectors and their completeness, independently of intrinsic Header facts. Authored
Reference GUIDs remain body contributions, while selected target metadata retains its actual
Header facts. Dangling and incompatible targets retain their original GUID and closed resolution
state through SourceRecordReference. A weak pointer whose original target is a Record retains
the existing supported Record-link interpretation and its actual pointer/header contributions.

The names collection uses SourcedValue<Vec<SourcedValue<LocationName>>>. Absence, authored null,
present empty, nullable slots, unreadable slots and repeated equal names remain distinct.
Scalar singleton normalization retains its original conversion. Tag operands and protected
GameTextRef lookups retain their existing domains and key/identity rules; the display-name
reference also retains the actual ancestor selection. Source-backed fields retain immediate
containsEncryptedSource and contributor references. DefinitionStatus remains a closed resolution
control within the directly sourced link.

There is one current wire form. No nullable-GUID mirror, parallel raw Header view or v2 writer is
introduced. The private original GUID and Record index retain existing sorting and physical ties;
they are not an additional wire dataset. Full Catalog and the selective SDK share this source
reader. The released v2 schema and fixture remain frozen. Original re-export, current schema
generation and updated consumers are required; an old export cannot reconstruct missing source
states, physical slots or source authority.
