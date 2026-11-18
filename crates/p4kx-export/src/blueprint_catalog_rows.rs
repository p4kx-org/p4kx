// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{DataForge, Guid, SourceUse, SourceUseRole, SourceValue, SourcedValue};

use crate::CatalogError;
use crate::blueprint_catalog_read::PreparedBlueprint;
use crate::blueprint_choice_inputs::BlueprintChoice;
use crate::blueprint_definition::{BlueprintDefinition, PhysicalBlueprintRef};
use crate::blueprint_presentation::{BlueprintDismantle, BlueprintPresentation};
use crate::blueprint_rule_inputs::BlueprintRules;
use crate::blueprint_tier_contract::BlueprintTier;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::export_ledger::SourceObservation;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;

#[path = "blueprint_category_knowledge.rs"]
mod category_knowledge;
use category_knowledge::CategoryKnowledge;

const SOURCE_TYPES: &[&str] = &["CraftingBlueprintRecord"];

pub(crate) type BlueprintRow<'source> = BlueprintRowData<BlueprintDefinitionReceipt<'source>>;

pub(crate) struct BlueprintDefinitionReceipt<'source> {
    pub(crate) physical_guid: CatalogGuid,
    pub(crate) physical_path: &'source str,
    pub(crate) numbers: Vec<CapturedNumber>,
}

pub(crate) struct BlueprintRowData<Definition> {
    pub(crate) definition: Definition,
    pub(crate) resources: crate::blueprint_backfill_source::BlueprintResources,
    tiers: Vec<BlueprintTier>,
    presentation: BlueprintPresentation<BlueprintChoice<CatalogGuid>>,
}

pub(crate) struct ProcessChoice<'rules> {
    pub(crate) guid: Option<Guid>,
    pub(crate) sources: &'rules [SourceUse],
    pub(crate) unavailable: bool,
}

impl<Definition> BlueprintRowData<Definition> {
    pub(crate) fn take_definition(self) -> (Definition, BlueprintRowData<()>) {
        let Self {
            definition,
            resources,
            tiers,
            presentation,
        } = self;
        (
            definition,
            BlueprintRowData {
                definition: (),
                resources,
                tiers,
                presentation,
            },
        )
    }
}

pub(crate) struct BlueprintTables {
    pub(crate) tiers: CatalogTable<BlueprintTier>,
    pub(crate) presentation: CatalogTable<BlueprintPresentation<SourcedValue<CatalogGuid>>>,
}

pub(crate) fn row<'source>(
    source: PreparedBlueprint<'source>,
    rules: &BlueprintRules,
    process: Option<Guid>,
    origins: &CatalogOrigins,
) -> Result<BlueprintRowData<BlueprintDefinition<'source>>, CatalogError> {
    row_with_process(source, rules, origins, || {
        Ok(ProcessChoice {
            guid: process,
            sources: &rules.processes,
            unavailable: rules.process_unavailable,
        })
    })
}

pub(crate) fn row_with_process<'source, 'rules>(
    source: PreparedBlueprint<'source>,
    rules: &BlueprintRules,
    origins: &CatalogOrigins,
    process: impl FnOnce() -> Result<ProcessChoice<'rules>, CatalogError>,
) -> Result<BlueprintRowData<BlueprintDefinition<'source>>, CatalogError> {
    let guid = source.definition.physical_guid.0;
    let mut default_uses = rules.defaults.sources.clone();
    default_uses.push(
        origins
            .record(source.record_index)
            .observe_header(RecordHeaderField::Guid, SourceUseRole::Selector)?,
    );
    let member = rules.defaults.members.get(&guid);
    default_uses.extend(member.into_iter().flatten().cloned());
    let default_available = origins.uses.value(
        SourceValue::Present(member.is_some()),
        &default_uses,
        !rules.defaults.unavailable,
    );
    let dismantle = dismantle_choice(&source, rules, process, origins)?;
    let owner = PhysicalBlueprintRef {
        input_ref: source.definition.input_ref.clone(),
        record_index: source.record_index,
    };
    let presentation = BlueprintPresentation {
        blueprint: owner,
        name: source.name,
        category: source.category,
        produced_item: source.produced_item.cell(origins),
        first_tier: source.first_tier,
        ingredients: source.ingredients,
        default_available,
        dismantle,
    };
    Ok(BlueprintRowData {
        definition: source.definition,
        resources: source.resources,
        tiers: source.tiers,
        presentation,
    })
}

/// Emits definitions in canonical GUID and physical order, then releases their source arenas.
pub(crate) fn retain_definitions<'source>(
    mut rows: Vec<BlueprintRowData<BlueprintDefinition<'source>>>,
    mut emit: impl FnMut(&BlueprintDefinition<'source>) -> Result<(), CatalogError>,
) -> Result<Vec<BlueprintRow<'source>>, CatalogError> {
    rows.sort_by_cached_key(|row| {
        (
            row.definition.physical_guid.text(),
            row.definition.physical_index,
        )
    });
    let mut retained = Vec::with_capacity(rows.len());
    for BlueprintRowData {
        definition,
        resources,
        tiers,
        presentation,
    } in rows
    {
        emit(&definition)?;
        retained.push(BlueprintRowData {
            definition: BlueprintDefinitionReceipt {
                physical_guid: definition.physical_guid,
                physical_path: definition.physical_path,
                numbers: definition.numbers,
            },
            resources,
            tiers,
            presentation,
        });
    }
    Ok(retained)
}

pub(crate) fn availability(forge: &DataForge<'_>, count: usize) -> DatasetAvailability {
    if count == 0 {
        DatasetAvailability::from_source_declarations(forge, |name| SOURCE_TYPES.contains(&name))
    } else {
        DatasetAvailability::Available
    }
}

fn dismantle_choice<'rules>(
    source: &PreparedBlueprint<'_>,
    rules: &BlueprintRules,
    process: impl FnOnce() -> Result<ProcessChoice<'rules>, CatalogError>,
    origins: &CatalogOrigins,
) -> Result<SourcedValue<BlueprintDismantle>, CatalogError> {
    let item = &source.produced_item;
    let SourceValue::Present(guid) = item.state else {
        return Ok(origins
            .uses
            .value(SourceValue::Absent, &item.used, item.complete));
    };
    let mut used = item.used.clone();
    if let Some(exclusion) = rules.item_exclusions.members.get(&guid.0) {
        used.extend(exclusion.iter().cloned());
        return Ok(origins
            .uses
            .value(SourceValue::Absent, &used, item.complete));
    }
    used.extend(rules.item_exclusions.sources.iter().cloned());
    let process = process()?;
    used.extend(process.sources.iter().cloned());
    let link = process.guid.map_or(SourceValue::Absent, |process_guid| {
        SourceValue::Present(BlueprintDismantle {
            process_guid: CatalogGuid(process_guid),
            tier: source.first_tier,
        })
    });
    Ok(origins.uses.value(
        link,
        &used,
        item.complete && !rules.item_exclusions.unavailable && !process.unavailable,
    ))
}

pub(crate) fn tables<Definition>(
    rows: Vec<BlueprintRowData<Definition>>,
    missing: &HashSet<Guid>,
    sources: &[SourceObservation],
    forge: &DataForge<'_>,
    origins: &CatalogOrigins,
) -> Result<BlueprintTables, CatalogError> {
    let wanted = rows
        .iter()
        .filter_map(|row| match &row.presentation.category.state {
            SourceValue::Present(guid) => Some(guid.0),
            _ => None,
        });
    let knowledge = CategoryKnowledge::read(missing, wanted, sources, origins)?;
    let mut tiers = Vec::new();
    let mut presentations = Vec::with_capacity(rows.len());
    for BlueprintRowData {
        tiers: recipe,
        presentation,
        ..
    } in rows
    {
        let BlueprintPresentation {
            blueprint,
            name,
            mut category,
            produced_item,
            first_tier,
            ingredients,
            default_available,
            dismantle,
        } = presentation;
        knowledge.finish(&mut category, origins)?;
        tiers.extend(recipe);
        presentations.push(BlueprintPresentation {
            blueprint,
            name,
            category: category.cell(origins),
            produced_item,
            first_tier,
            ingredients,
            default_available,
            dismantle,
        });
    }
    Ok(BlueprintTables {
        tiers: CatalogTable::new(forge, SOURCE_TYPES, tiers),
        presentation: CatalogTable::new(forge, SOURCE_TYPES, presentations),
    })
}

#[cfg(test)]
#[path = "blueprint_definition_staging_tests.rs"]
mod staging_tests;
