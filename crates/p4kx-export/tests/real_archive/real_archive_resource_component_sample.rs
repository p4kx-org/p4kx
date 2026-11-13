// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, RecordTree, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use super::{Coverage, EXPECTED, SCALARS, TABLE, TestResult, compare_component, representative};
use crate::authored_definition_witness::field;
use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};
use crate::real_archive_item_components as components;
use crate::real_archive_resource_network as network;
use crate::real_archive_verification::print_line;

const RESOURCE: &str = "ItemResourceComponentParams";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn source_selected_resource_components_and_game_mode_rules_match_catalog() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let selected = select_items(&forge, &watch)?;
    for role in EXPECTED {
        let Some(tree) = selected.get(role) else {
            print_line(&format!(
                "Resource component {role}: no original owner observed"
            ));
            continue;
        };
        let catalog = build(&archive, &entry, tree)?;
        let origins = Origins::read(&catalog, entry.origin())?;
        let count = compare_item(&catalog, &origins, tree)?;
        print_line(&format!(
            "Resource component {role}: {count} original physical slots compared"
        ));
    }
    if let Some(tree) = select_mode(&forge, &watch)? {
        let catalog = build(&archive, &entry, &tree)?;
        let origins = Origins::read(&catalog, entry.origin())?;
        compare_mode(&catalog, &origins, &tree)?;
    } else {
        print_line("ResourceNetworkParams: no original GameMode owner observed");
    }
    watch
        .check()
        .map_err(|_| "Archive changed during verification".into())
}

fn select_items<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<BTreeMap<&'static str, RecordTree<'source>>> {
    let mut selected = BTreeMap::new();
    let Some(probe) = forge.pointer_probe("Components", RESOURCE) else {
        return Ok(selected);
    };
    let mut builder = TreeBuilder::new();
    let rule = Kind::Items.rule().ok_or("selected Item rule")?;
    let filter = Filter::new(components::FILTER);
    for record in forge.records() {
        watch.poll().map_err(|_| "Archive changed")?;
        if !components::selected_item(&record, &rule, &filter)?
            || !builder
                .holds(&probe, &record)
                .map_err(|_| "inspect original resource slots")?
        {
            continue;
        }
        let tree = builder
            .build(&record)
            .map_err(|_| "read original resource owner")?;
        if let Some(role) = representative(&tree) {
            selected.entry(role).or_insert(tree);
        }
        if selected.len() == EXPECTED.len() {
            break;
        }
    }
    Ok(selected)
}

fn compare_item(
    catalog: &Value,
    origins: &Origins<'_>,
    tree: &RecordTree<'_>,
) -> TestResult<usize> {
    let mut actual = components::component_rows(catalog, TABLE)?;
    let Some(Original::Array(slots)) = tree.field("Components") else {
        return Err("original Components array".into());
    };
    let locations = Locations::read(tree.root());
    let owner = tree.record_index();
    let mut coverage = Coverage::new();
    let mut count = 0;
    for (slot, raw) in slots.iter().enumerate() {
        let Original::Struct(source) = raw else {
            continue;
        };
        if source.type_name() != RESOURCE {
            continue;
        }
        let row = actual
            .remove(&(owner, slot))
            .ok_or("missing original resource component")?;
        if row.pointer("/ownerGuid/value").and_then(Value::as_str)
            != Some(tree.guid().to_string().as_str())
            || row.pointer("/sourceType/value").and_then(Value::as_str) != Some(RESOURCE)
        {
            return Err("resource component physical owner differs".into());
        }
        origins.occurrence(
            field(row, "sourceType")?,
            owner,
            locations.ordinal(raw)?,
            "$type",
        )?;
        compare_component(source, row, &mut coverage)?;
        for name in SCALARS {
            if let Some(raw) = source.field(name) {
                origins.occurrence(field(row, name)?, owner, locations.ordinal(raw)?, "$value")?;
            }
        }
        count += 1;
    }
    if count == 0 || actual.keys().any(|(index, _)| *index == owner) {
        return Err("resource rows do not match the original component slots".into());
    }
    for (field, coverage) in coverage {
        print_line(&format!(
            "Resource component {field}: {} stored values, {} missing slots",
            coverage.values, coverage.missing
        ));
    }
    Ok(count)
}

fn select_mode<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<Option<RecordTree<'source>>> {
    let mut builder = TreeBuilder::new();
    let mut without_rules = None;
    for record in forge.records() {
        watch.poll().map_err(|_| "Archive changed")?;
        if record.type_name()? != "GameMode" {
            continue;
        }
        let tree = builder
            .build(&record)
            .map_err(|_| "read original GameMode owner")?;
        if rule_block(&tree).is_some() {
            return Ok(Some(tree));
        }
        without_rules.get_or_insert(tree);
    }
    Ok(without_rules)
}

fn rule_block<'read, 'source>(
    tree: &'read RecordTree<'source>,
) -> Option<&'read Original<'source>> {
    let Some(Original::Struct(params)) = tree.field("params") else {
        return None;
    };
    params
        .field("resourceNetworkParams")
        .filter(|rules| matches!(rules, Original::Struct(_)))
}

fn compare_mode(catalog: &Value, origins: &Origins<'_>, tree: &RecordTree<'_>) -> TestResult {
    let index = u64::from(tree.record_index());
    let rows = catalog
        .pointer(network::TABLE)
        .and_then(Value::as_array)
        .ok_or("GameMode resource table")?;
    let mut owners = rows.iter().filter(|row| {
        row.pointer("/source/recordIndex/value")
            .and_then(Value::as_u64)
            == Some(index)
    });
    let row = owners.next().ok_or("missing original GameMode owner")?;
    if owners.next().is_some() {
        return Err("duplicate original GameMode owner".into());
    }
    if !network::compare_mode(tree, row)? {
        print_line(
            "ResourceNetworkParams: no original rule block observed; the selected GameMode state matches",
        );
        return Ok(());
    }
    let raw @ Original::Struct(rules) =
        rule_block(tree).ok_or("original ResourceNetworkParams block")?
    else {
        return Err("original ResourceNetworkParams shape".into());
    };
    let owner = tree.record_index();
    let locations = Locations::read(tree.root());
    let carrier = row
        .pointer("/params/value/resourceNetworkParams")
        .ok_or("original rules carrier")?;
    origins.occurrence(carrier, owner, locations.ordinal(raw)?, "$type")?;
    let output = field(carrier, "value")?;
    for name in network::RULE_FIELDS {
        if let Some(raw) = rules.field(name) {
            origins.occurrence(
                field(output, name)?,
                owner,
                locations.ordinal(raw)?,
                "$value",
            )?;
        }
    }
    print_line(
        "ResourceNetworkParams: original GameMode owner, rule values and exact field origins compared",
    );
    Ok(())
}

fn build(archive: &Archive, entry: &DataForgeEntry, tree: &RecordTree<'_>) -> TestResult<Value> {
    let filter = Filter::new(tree.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "build source-selected resource catalog")?;
    let catalog: Value =
        serde_json::from_slice(built.json()).map_err(|_| "read scoped catalog JSON")?;
    crate::real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    Ok(catalog)
}
