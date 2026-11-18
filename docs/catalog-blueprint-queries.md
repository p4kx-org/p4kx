# Selective physical Blueprints

Use `p4kx_export::catalog_queries::with_blueprints` or `with_blueprints_from` to read complete
canonical Blueprint definitions, all their tiers and their presentation through a callback. The
`_from` form accepts an already decoded Entry of the same Archive. Session and parsed-session
methods reuse the Archive binding, decoded DataForge, requested-locale policy and lazy indexes.
No `read_crafting`, complete Catalog, JSON adapter or world-placement walk is required.

`CatalogRecordSelection::All` selects the ordinary Blueprint family. `Guid` preserves every
eligible physical declaration of that GUID; `RecordIndex` disambiguates one owner in the current
DataForge. Development leftovers retain the existing exclusion policy. Definitions follow textual
GUID order, with physical index for equal GUIDs. Tiers and presentation use the same physical order.

The snapshot supplies these unchanged canonical datasets:

| Getter | Catalog dataset |
| --- | --- |
| `definitions()` | `definitions.blueprints.rows` |
| `tiers()` | `configurations.blueprintTiers.rows` |
| `presentations()` | `projections.blueprintPresentation.rows` |

A definition provides its sourced header, `input_ref()` and complete `source()` arena. Follow
ordered `roots()` and `values()`, resolving each `SourceValueRef` through that owner's arena.
Source fields preserve declarations, original positions, duplicate occurrences, nulls, reference
resolution and original errors. A reference from one physical Blueprint must never be resolved
against another Blueprint's arena, including an owner with the same external GUID.

Every tier exposes its physical owner, original `tier_index()`, costs, mandatory and optional
links, craft time and research envelope/pointers. Costs retain their source link, sourced position,
full modifier context, original quantity/composition links and typed meaning. Modifier and range
getters preserve every authored occurrence, range kind, bounds and original-value navigation.
Unrecognized source range types additionally appear in `unsupported_ranges()` with their physical
owner. They are not silently assigned a formula.

The quality exchange contract remains [formula version1](catalog-combat-quality.md). Reconstruct
`slotPath` from each cost's position: mandatory or optional(index) first, then option(index)
steps through the referenced parent cost in the same tier. Tier and path indices are original
zero-based array positions, including gaps left by skipped/null siblings. `debugName` is a label;
`SourceValueRef.valueIndex` is local navigation. Neither replaces the stable exchange path.

Presentation retains protected names, category and produced-item choices, first-tier ingredient
navigation, default availability and generic dismantle selection. Defaults and blacklists require
the complete relevant CraftingGlobalParams family. Dismantle source bodies are prepared once,
only if the canonical selected-item/blacklist interpreter actually needs them. Exactly one live
physical dismantle candidate is required; repeated GUIDs do not collapse physical candidates.
Category bodies are limited to GUIDs actually referenced by selected owners, with the canonical
failed/skipped-candidate finalization. Referenced item, resource, GameplayProperty and quality
bodies are not loaded merely to expose their original arena/header links.

Check `coverage()` and each axis of `dependency_coverage()` before replacing a consumer cache.
The axes distinguish globals, demanded dismantle candidates and referenced categories from owner
header/body/field issues. Availability describes family declarations, so an empty selection does
not by itself prove family absence. Unclassifiable headers are coverage gaps, not invented
Category identities. Inspect returned per-datum source states and incomplete origins as well.

`game_text()` and `public_name()` resolve only this result's text references. Blueprint names and
selection labels retain their protected text domain. Requested languages include absent payloads;
available languages list only requested payloads actually read. Empty/failed owner selections
read no localization or rule payloads. Per-datum encrypted, plain and unknown origins remain
independent of text domain. Resolve uses and physical ancestors through `source_inventory()`.

Borrowed definitions and text rows stay inside the callback. Build consumer-owned results there,
then publish them only after the outer query succeeds: cancellation and Archive-change checks can
still reject the operation after the callback. No fixed source-count quota is added. Asking for All
materializes that selected Blueprint family; requesting one physical owner avoids sibling bodies.
Independent crafting/global/quality/reward and other SDK domains remain separate endpoints.

Decision: [ADR0405](adr/0405-direct-blueprint-queries-share-canonical-rule-and-slot-interpretation.md).
