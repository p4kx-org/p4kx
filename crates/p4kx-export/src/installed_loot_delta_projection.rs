// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, DataForge, DataForgeEntry, FieldOrigin, PhysicalSourceKind, Record, RecordTree,
    SourceInventory, SourceLocation, SourceUseRole,
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::Kind;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::DatasetAvailability;
use crate::catalog_queries::loot::{
    LootDefinition, LootDefinitionKind, LootDefinitionSnapshot, LootTable,
};
use crate::loot_kinds::{LootKinds, LootPick};
use crate::resource_number::ResourceNumber;
use crate::source_value_catalog::SourcePayload;
use crate::texts::{TextSources, Texts};

use super::fixture::{DataForgeIdentity, Side};
use super::{OriginalEntry, TestResult};

pub(super) struct Produced {
    definition: Value,
    weighted: Value,
}

pub(super) fn producer<'source>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &'source DataForge<'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    side: &Side,
    original: &OriginalEntry,
) -> TestResult<Produced> {
    let text_sources = TextSources::read_languages(archive, &[])
        .map_err(|_| "could not prepare empty projection localization")?;
    let texts = Texts::parse(&text_sources)
        .map_err(|_| "could not initialize the scoped projection text index")?;
    let origins = CatalogOrigins::for_archive_records(archive, entry)
        .map_err(|_| "could not admit scoped producer Archive origin")?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(Kind::LootTables);
    let pick = LootPick::read_weighted(LootDefinitionKind::Table, record, tree, &projection, None)
        .map_err(|_| "scoped normal Loot producer failed")?
        .ok_or("scoped normal Loot producer returned no owner")?;
    let LootPick::Table(table, definition) = &pick else {
        return Err("scoped normal Loot producer returned the wrong family".into());
    };
    let inventory = origins
        .uses
        .freeze()
        .map_err(|_| "producer source freeze failed")?;
    check_entry(definition, table, &inventory, &side.dataforge, original)?;
    let mut output = LootKinds::default();
    output.push(pick);
    output.order();
    let mut definitions = wire(&output.definitions.tables(forge))?;
    let definition = definitions
        .as_object_mut()
        .and_then(|tables| tables.remove("lootTables"))
        .ok_or("scoped normal producer did not emit the canonical table")?;
    Ok(Produced {
        definition,
        weighted: wire(&output.tables)?,
    })
}

pub(super) fn compare(
    snapshot: &LootDefinitionSnapshot<'_>,
    produced: &Produced,
    side: &Side,
    original: &OriginalEntry,
) -> TestResult {
    let ([definition], [weighted]) = (snapshot.rows.as_slice(), snapshot.tables.as_slice()) else {
        return Err("physical Loot SDK selection did not produce one original owner".into());
    };
    if !matches!(snapshot.availability, DatasetAvailability::Available)
        || !snapshot.coverage.unreadable_headers.is_empty()
        || !snapshot.coverage.unreadable_bodies.is_empty()
        || !snapshot.archetypes.is_empty()
    {
        return Err("incomplete or unexpected coverage for selected Loot owner".into());
    }
    check_entry(
        definition,
        weighted,
        &snapshot.source_inventory,
        &side.dataforge,
        original,
    )?;
    let definition = json!({
        "availability": snapshot.availability,
        "rows": wire(&snapshot.rows)?
    });
    if definition != produced.definition || wire(&snapshot.tables)? != produced.weighted {
        return Err("scoped normal producer and SDK disagree on the serialized Loot owner".into());
    }
    Ok(())
}

fn check_entry(
    definition: &LootDefinition<'_>,
    table: &LootTable<'_>,
    source_inventory: &SourceInventory,
    dataforge: &DataForgeIdentity,
    original: &OriginalEntry,
) -> TestResult {
    if definition.source().record_index().value().copied() != Some(original.owner)
        || table.record_index != original.owner
    {
        return Err("canonical row and convenience view do not join the original owner".into());
    }
    let entry = table
        .entries
        .get(original.projected_entry)
        .ok_or("projected entry missing")?;
    let selected = entry
        .weight_source
        .value()
        .copied()
        .ok_or("selected weight occurrence missing")?;
    if selected.value_index() != original.ordinal
        || entry.weight.map(f32::to_bits) != Some(original.bits)
    {
        return Err("convenience weight lost the original position or bits".into());
    }
    let raw = definition
        .value(selected)
        .ok_or("owning definition lost its exact original slot")?
        .value();
    let Some(SourcePayload::Number {
        value: ResourceNumber::Single(value),
    }) = raw.value()
    else {
        return Err("canonical Loot value changed the original numeric width".into());
    };
    if value.raw_bits() != original.bits {
        return Err("canonical Loot value changed the original IEEE bits".into());
    }
    for origin in [raw.origin(), entry.weight_source.origin()] {
        if !has_exact_use(
            origin,
            source_inventory,
            dataforge,
            original.owner,
            original.ordinal,
            SourceUseRole::Payload,
        ) {
            return Err("weight cell lacks an exact original DataForge source use".into());
        }
    }
    original.optional.check(
        definition,
        entry,
        source_inventory,
        dataforge,
        original.owner,
    )
}

pub(super) fn has_exact_use(
    origin: &FieldOrigin,
    inventory: &SourceInventory,
    dataforge: &DataForgeIdentity,
    owner: u32,
    ordinal: u64,
    role: SourceUseRole,
) -> bool {
    if !origin.source_set_complete() || origin.contains_encrypted_source() != dataforge.encrypted {
        return false;
    }
    let observed = origin.source_uses_refs();
    inventory.iter_uses().any(|used| {
        observed.contains(used.id())
            && used.role() == role
            && matches!(
                used.location(),
                SourceLocation::Subresource { namespace, indices, field }
                    if namespace == "dataforgeValue"
                        && indices.as_slice() == [u64::from(owner), ordinal]
                        && field == "$value"
            )
            && inventory.iter_sources().any(|source| {
                source.id() == used.source()
                    && matches!(source.kind(), PhysicalSourceKind::ArchiveEntry(entry)
                        if u32::try_from(entry.entry_index()).ok() == Some(dataforge.entry_index)
                            && entry.path().value() == Some(&dataforge.path)
                            && entry.record_range().offset.get() == dataforge.record_offset
                            && entry.record_range().length.get() == dataforge.record_length)
            })
    })
}

fn wire(value: &impl Serialize) -> TestResult<Value> {
    let bytes = serde_json::to_vec(value).map_err(|_| "serializing selected Loot row failed")?;
    serde_json::from_slice(&bytes).map_err(|_| "reading selected Loot row wire form failed".into())
}
