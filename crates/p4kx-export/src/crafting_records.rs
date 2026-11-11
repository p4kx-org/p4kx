// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Guid, RecordTree, StructValue, Value};

use crate::CatalogError;
use crate::blueprint::{as_struct, craft_time};
use crate::crafting_tier;
use crate::crafting_types::{
    BlueprintPool, BlueprintTiers, DismantleProcess, GameplayProperty, GlobalParams,
    LocationOverride, PoolEntry, QualityDistribution, Tier,
};
use crate::quality_curve_read;
use crate::record_fields::{class_name, elements, locale_key, number, reference, structure};
use crate::selection::{Kind, KindRule, starts_with_folder};
use crate::source_fields::{self, FieldError, Fields};

const ROOT: &str = "libs/foundry/records/crafting/";
const DISMANTLE_PROCESS: &str = "GenericCraftingProcess_Dismantle";

pub(crate) const DISMANTLES: KindRule = Kind::DismantleProcesses.records();
pub(crate) const PROPERTIES: KindRule = Kind::GameplayProperties.records();
pub(crate) const DISTRIBUTIONS: KindRule = Kind::QualityDistributions.records();
pub(crate) const OVERRIDES: KindRule = KindRule {
    prefixes: DISTRIBUTIONS.prefixes,
    type_name: "CraftingQualityLocationOverrideRecord",
};
pub(crate) const POOLS: KindRule = Kind::BlueprintPools.records();
pub(crate) const GLOBALS: KindRule = KindRule {
    prefixes: &["libs/foundry/records/crafting/globalparams/"],
    type_name: "CraftingGlobalParams",
};

/// Returns whether `record_path` can belong to any crafting rule.
pub(crate) fn is_crafting_path(record_path: &str) -> bool {
    starts_with_folder(record_path, ROOT)
}

/// Reads the interpreted public Blueprint from its canonical Record.
pub(crate) fn blueprint<'source>(
    tree: &RecordTree<'source>,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintTiers<'source>, CatalogError> {
    let source = blueprint_source(tree);
    let mut unknown = Vec::new();
    let tiers = crafting_tier::tiers(source, &mut unknown, cancel)?
        .into_iter()
        .map(|tier| tier.tier)
        .collect();
    Ok(blueprint_metadata(tree, source, tiers, unknown))
}

pub(crate) fn blueprint_source<'read, 'source>(
    tree: &'read RecordTree<'source>,
) -> Option<&'read StructValue<'source>> {
    structure(Some(tree.root()), "blueprint")
}

fn blueprint_metadata<'source>(
    tree: &RecordTree<'source>,
    blueprint: Option<&StructValue<'source>>,
    tiers: Vec<Tier<'source>>,
    unknown_range_types: Vec<&'source str>,
) -> BlueprintTiers<'source> {
    BlueprintTiers {
        guid: tree.guid(),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        name_key: locale_key(blueprint, "blueprintName"),
        category: reference(blueprint, "category").map(|guid| guid.0),
        produced_item: reference(structure(blueprint, "processSpecificData"), "entityClass")
            .map(|guid| guid.0),
        tiers,
        unknown_range_types,
    }
}

/// Returns the dismantle process the Record describes, `None` for another process.
pub(crate) fn dismantle<'f>(tree: &RecordTree<'f>) -> Option<DismantleProcess<'f>> {
    let blueprint = structure(Some(tree.root()), "blueprint");
    let process = structure(blueprint, "processSpecificData")
        .filter(|process| process.type_name() == DISMANTLE_PROCESS)?;
    Some(DismantleProcess {
        guid: tree.guid(),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        efficiency: number(Some(process), "efficiency"),
        time_seconds: craft_time(structure(Some(process), "dismantleTime")),
    })
}

pub(crate) fn property<'f>(tree: &RecordTree<'f>) -> GameplayProperty<'f> {
    GameplayProperty {
        guid: tree.guid(),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        name_key: locale_key(Some(tree.root()), "propertyName"),
    }
}

pub(crate) struct DistributionRead<'source> {
    pub(crate) value: QualityDistribution<'source>,
    pub(crate) notes: Vec<FieldError>,
}

pub(crate) fn distribution<'source>(
    tree: &RecordTree<'source>,
) -> Result<DistributionRead<'source>, FieldError> {
    let fields = Fields::new(tree.root(), "");
    let mut notes = Vec::new();
    let distribution = fields
        .structure("qualityDistribution")?
        .map(|source| quality_curve_read::read(source, "qualityDistribution", &mut notes))
        .transpose()?;
    let overrides = fields
        .structure("locationOverride")?
        .map(|source| quality_overrides(source, &mut notes))
        .transpose()?
        .unwrap_or_default();
    Ok(DistributionRead {
        value: QualityDistribution {
            guid: tree.guid(),
            class_name: class_name(tree),
            record_path: tree.record_path(),
            distribution,
            overrides,
        },
        notes,
    })
}

fn quality_overrides<'source>(
    source: &StructValue<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<Vec<LocationOverride<'source>>, FieldError> {
    let fields = Fields::new(source, "locationOverride");
    fields.check(&["locationOverrideList"], notes);
    let mut overrides = Vec::new();
    for (index, value) in fields
        .array("locationOverrideList")?
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        let path = format!("locationOverride.locationOverrideList[{index}]");
        let entry = source_fields::entry(value, source.type_name(), &path)?.ok_or_else(|| {
            fields.invalid(
                &format!("locationOverrideList[{index}]"),
                value,
                "a non-null quality override entry",
            )
        })?;
        let entry_fields = Fields::new(entry, &path);
        entry_fields.check(&["location", "qualityDistribution"], notes);
        let distribution = entry_fields
            .structure("qualityDistribution")?
            .map(|source| {
                quality_curve_read::read(source, &format!("{path}.qualityDistribution"), notes)
            })
            .transpose()?;
        overrides.push(LocationOverride {
            location: entry_fields.reference("location")?.map(|link| link.guid.0),
            distribution,
        });
    }
    Ok(overrides)
}

pub(crate) fn pool<'f>(tree: &RecordTree<'f>) -> BlueprintPool<'f> {
    BlueprintPool {
        guid: tree.guid(),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        entries: elements(Some(tree.root()), "blueprintRewards")
            .iter()
            .filter_map(as_struct)
            .map(|entry| PoolEntry {
                blueprint: reference(Some(entry), "blueprintRecord").map(|guid| guid.0),
                weight: number(Some(entry), "weight"),
            })
            .collect(),
    }
}

pub(crate) fn globals(tree: &RecordTree<'_>) -> GlobalParams {
    let root = Some(tree.root());
    let selection = structure(root, "defaultBlueprintSelection");
    GlobalParams {
        default_blueprints: references(selection, "blueprintRecords"),
        dismantle_blacklist_resources: references(root, "dismantleBlacklistResources"),
        dismantle_blacklist_items: references(root, "dismantleBlacklistEntityClasses"),
    }
}

fn references(parent: Option<&StructValue<'_>>, name: &str) -> Vec<Guid> {
    elements(parent, name)
        .iter()
        .filter_map(|value| match value {
            Value::Reference(reference) if reference.location().is_none() => Some(reference.guid()),
            _ => None,
        })
        .collect()
}
