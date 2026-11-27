# Authored blueprint category lists

Schema 3 exports the ordered database declarations in
`declarations.blueprintCategoryLists: CatalogTable<BlueprintCategoryList>`.
The independent Kind identifier is `blueprintCategoryLists`, appended at position50.
Selection uses exact type `BlueprintCategoryDatabaseRecord` at every original path,
with the ordinary Entity Filter and development-leftover policy. It has its own
completeness and source/read/projection counters. The global Tag Filter exception
does not apply.

Each physical selected readable owner keeps the shared eight sourced
[Record header](catalog-record-headers.md) fields and one `categories` field.
The latter is `SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>`:
original order, repetitions, nulls, dangling values, foreign physical targets and
all absent/null/present/unreadable/invalid states survive. Equal nominal database
GUIDs retain distinct physical Record indices and stable original tie order.
An empty present array stays distinct from absent/null or failed reading.

The array and each original `categories[n]` slot use the owning database's actual
body coordinates. The authored GUID uses that body slot; resolved target index/type
use the actual target's header selectors, and its record path uses the original
filename conversion. Each cell immediately carries containsEncryptedSource,
sourceUsesRefs and sourceSetComplete. An untouched target body contributes nothing.
A standalone DataForge input remains unknown without original P4K lineage.

These are raw authored references. Source resolution retains its actual status
independently of which category definitions were selected or emitted. A filtered,
foreign or same-GUID physical target is neither replaced nor automatically included.
There is no category catalog-materialization join, hierarchy, inferred name or
category-rule synthesis. Existing Resource/Commodity reference validation and GUID
ambiguity policy remain unchanged.

A readable unknown body field retains its original header in diagnostic output,
with an exact UnsupportedValue diagnosis and normal-publication refusal.
Malformed or unreadable categories retain their typed state and field diagnosis.
A strict canonical tree failure emits no list row and records one source-read failure.
Family availability still describes the declared source type, independently of the
selected or successfully emitted row count. A Filter excludes a broken owner before
reading it.

The separate `definitions.blueprintCategories` table remains fieldless category
definitions. Lists neither change its counts nor create missing definitions.
Blueprint category unlinking consults actual selected/selectionCandidate failed
category-definition observations; a failed database or other source with the same
nominal GUID cannot clear a valid category link. Existing skipped-category handling
is unchanged. The interpreted public read_crafting API and Commodity backfill keep
their existing meaning.

This is a schema3-only writer contract; no flat duplicate dataset or v2 adapter is
provided. Generated schema and current CLI snapshots bind the required A table and
ordinary filtered Kind scope.
