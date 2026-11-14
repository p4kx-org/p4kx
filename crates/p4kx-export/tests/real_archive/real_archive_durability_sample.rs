// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Filter, InputScope,
    RecordTree, SourceEncryption, SourceInput, SourceUses, StructValue, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use super::{Result, Trace, indexed};
use crate::real_archive_item_components::{self as components, cell};
use crate::real_archive_shop_inventory_v3;
use crate::real_archive_verification::print_line;

const HEALTH: &str = "SHealthComponentParams";
const SHIELD: &str = "SCItemShieldGeneratorParams";
const ARMOUR: &str = "SCItemVehicleArmorParams";
const ROWS: &str = "/configurations/durability/rows";

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Category {
    HealthOnly,
    Shield,
    Armour,
}

impl Category {
    const fn label(self) -> &'static str {
        match self {
            Self::HealthOnly => "health-only",
            Self::Shield => "shield",
            Self::Armour => "armour",
        }
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn source_selected_health_only_shield_and_armour_match_original_durability() -> Result {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let selected = select(&forge, &watch)?;
    for category in [Category::HealthOnly, Category::Shield, Category::Armour] {
        let tree = selected.get(&category).ok_or_else(|| {
            format!(
                "no representative original {} Health owner observed",
                category.label()
            )
        })?;
        let catalog = build(&archive, &entry, tree)?;
        let (slots, fields) = compare(&catalog, entry.origin(), tree, category)?;
        print_line(&format!(
            "Durability {}: {slots} physical Health slots, {fields} original scalar fields and origins",
            category.label()
        ));
    }
    watch
        .check()
        .map_err(|_| "Archive changed during verification".into())
}

fn select<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> Result<BTreeMap<Category, RecordTree<'source>>> {
    let rule = Kind::Items.rule().ok_or("selected Item rule")?;
    let filter = Filter::new(components::FILTER);
    let probe = forge
        .pointer_probe("Components", HEALTH)
        .ok_or("original Health declaration")?;
    let mut trees = TreeBuilder::new();
    let mut selected = BTreeMap::new();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during source selection")?;
        if !components::selected_item(&record, &rule, &filter)?
            || !components::has_component(&mut trees, &record, std::slice::from_ref(&probe))?
        {
            continue;
        }
        let tree = trees
            .build(&record)
            .map_err(|_| "read original Health owner")?;
        if let Some(category) = classify(&tree)? {
            selected.entry(category).or_insert(tree);
        }
        if selected.len() == 3 {
            break;
        }
    }
    Ok(selected)
}

fn classify(tree: &RecordTree<'_>) -> Result<Option<Category>> {
    let slots = original_slots(tree)?;
    let components = || {
        slots.iter().filter_map(|value| match value {
            Original::Struct(component) => Some(component),
            _ => None,
        })
    };
    if !components().any(|component| {
        component.type_name() == HEALTH
            && matches!(component.field("Health"), Some(Original::Single(_)))
    }) {
        return Ok(None);
    }
    let has = |source| components().any(|component| component.type_name() == source);
    let attach_type = components()
        .filter(|component| component.type_name() == "SAttachableComponentParams")
        .find_map(attach_type);
    Ok(if has(SHIELD) {
        Some(Category::Shield)
    } else if has(ARMOUR) || (attach_type == Some("Armor") && has("SCItemSuitArmorParams")) {
        Some(Category::Armour)
    } else if !slots.iter().any(|slot| matches!(slot, Original::Error(_)))
        && !components().any(|component| specialist(component.type_name()))
        && !attach_type.is_some_and(|kind| {
            kind.eq_ignore_ascii_case("PowerPlant") || kind.eq_ignore_ascii_case("Cooler")
        })
    {
        Some(Category::HealthOnly)
    } else {
        None
    })
}

fn specialist(source_type: &str) -> bool {
    matches!(
        source_type,
        "SCItemWeaponComponentParams"
            | SHIELD
            | "SCItemQuantumDriveParams"
            | "SCItemMissileParams"
            | "SCItemSuitArmorParams"
            | "SCItemClothingParams"
            | "SCItemRadarComponentParams"
            | "EntityComponentAttachableModifierParams"
            | "SEntityComponentMiningLaserParams"
    )
}

fn attach_type<'source>(component: &StructValue<'source>) -> Option<&'source str> {
    let Original::Struct(attach) = component.field("AttachDef")? else {
        return None;
    };
    match attach.field("Type")? {
        Original::String(kind) | Original::Locale(kind) | Original::Enum(kind) => Some(kind),
        _ => None,
    }
}

fn original_slots<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> Result<&'tree [Original<'source>]> {
    let Some(Original::Array(slots)) = tree.field("Components") else {
        return Err("the original Health owner must have Components".into());
    };
    Ok(slots)
}

fn build(archive: &Archive, entry: &DataForgeEntry, tree: &RecordTree<'_>) -> Result<Value> {
    let filter = Filter::new(tree.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "build selected Health owner catalog")?;
    let items = built
        .summary()
        .export_health()
        .kind(Kind::Items)
        .ok_or("selected Item health")?;
    assert!(
        items.source_inventory_complete
            && items.field_inventory_complete
            && items.failed_records().is_empty()
            && items.failed_entries().is_empty(),
        "the selected Item scope has a complete source inventory with no failed reads"
    );
    let catalog = serde_json::from_slice(built.json()).map_err(|_| "read scoped catalog JSON")?;
    real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    Ok(catalog)
}

fn compare(
    catalog: &Value,
    origin: &DataForgeOrigin,
    tree: &RecordTree<'_>,
    category: Category,
) -> Result<(usize, usize)> {
    let DataForgeOrigin::Archive(entry) = origin else {
        return Err("expected observed Archive DataForge".into());
    };
    let uses = indexed(catalog, "/provenance/uses", "id")?;
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(entry), InputScope::Payload)?;
    let encrypted = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let trace = Trace::of(tree, &uses, &scope, encrypted);
    let guid = tree.guid().to_string();
    let item = indexed(catalog, "/items", "guid")?
        .get(guid.as_str())
        .copied()
        .ok_or("the physical Health owner has an Item row")?;
    if category == Category::HealthOnly {
        assert!(
            cell(item, "stats")?.is_null(),
            "health-only Item remains outside specialist stats"
        );
    }
    let mut rows = components::component_rows(catalog, ROWS)?;
    let mut slots = 0;
    let mut fields = 0;
    for (slot, raw) in original_slots(tree)?.iter().enumerate() {
        let Original::Struct(health) = raw else {
            continue;
        };
        if health.type_name() != HEALTH {
            continue;
        }
        let row = rows
            .remove(&(tree.record_index(), slot))
            .ok_or("one row per physical Health slot")?;
        assert!(
            super::text(row, "/ownerGuid/value")? == guid,
            "the original Record GUID matches"
        );
        trace.header(cell(row, "ownerGuid")?, "guid", "payload")?;
        trace.header(cell(row, "ownerRecordIndex")?, "recordIndex", "selector")?;
        trace.check(cell(row, "componentOrdinal")?, raw, "conversion")?;
        slots += 1;
        fields += trace.health(row, health)?;
    }
    assert!(
        !rows.keys().any(|(owner, _)| *owner == tree.record_index()) && slots > 0 && fields > 0,
        "all Health rows for the selected owner match its physical slots and the sample has fields ({slots} slots, {fields} fields, {} rows for other paths selected by the Filter)",
        rows.len()
    );
    Ok((slots, fields))
}
