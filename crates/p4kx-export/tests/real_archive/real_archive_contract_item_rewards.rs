// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, RecordTree, Reference, StructValue, Value,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value as Json;

use crate::real_archive_auto_spawns::original::Locations;
use crate::real_archive_auto_spawns::provenance::Origins;
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_verification::print_line;

const OWNER: &str = "P4KX_ITEM_REWARD_GENERATOR_RECORD";

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn selected_item_reward_generator_matches_its_original_occurrences() -> TestResult {
    let record_path = std::env::var(OWNER)
        .map_err(|_| "set P4KX_ITEM_REWARD_GENERATOR_RECORD to one original Generator path")?;
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let source = DataForgeEntry::read(&archive)?;
    let forge = source.open()?;
    let mut chosen = None;
    for record in forge.records() {
        watch.poll()?;
        if record.type_name()? == "ContractGenerator"
            && record.record_path()?.eq_ignore_ascii_case(&record_path)
            && chosen.replace(record).is_some()
        {
            return Err("expected exactly one physical Generator record".into());
        }
    }
    let tree = chosen.ok_or("the selected Generator is present")?.tree()?;
    let filter = Filter::new(&record_path);
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(&archive, &source, options.with_full_provenance())
        .map_err(|_| "build the selected Generator Catalog")?;
    let catalog: Json = serde_json::from_slice(built.json())?;
    verify_build(&archive, &catalog)?;
    let count = compare(&tree, &catalog, &Origins::read(&catalog, source.origin())?)?;
    watch.check()?;
    print_line(&format!("physical contract item results compared: {count}"));
    Ok(())
}

fn compare(tree: &RecordTree<'_>, catalog: &Json, origins: &Origins<'_>) -> TestResult<usize> {
    let rows = catalog
        .pointer("/definitions/contractGenerators/rows")
        .and_then(Json::as_array)
        .ok_or("published Generator rows")?;
    let index = tree.record_index();
    let definition = rows
        .iter()
        .find(|row| row.pointer("/recordIndex/value") == Some(&Json::from(index)))
        .ok_or("the exact Generator definition is present")?;
    let output = definition
        .get("itemRewardResults")
        .and_then(Json::as_array)
        .ok_or("item reward results")?;
    let locations = Locations::read(tree.root());
    let mut count = 0;
    let mut instances = 0;
    let mut failure = None;
    tree.root().visit_owned_values(|visit| {
        if failure.is_some() || visit.field_name() != "contractResults" {
            return;
        }
        let Value::Struct(result) = visit.value() else {
            return;
        };
        if result.type_name() != "ContractResult_Item" {
            return;
        }
        match output
            .get(count)
            .ok_or("missing item reward occurrence")
            .map_err(Box::<dyn Error>::from)
            .and_then(|row| compare_row(tree, visit.ordinal(), result, &locations, row, origins))
        {
            Ok(instance) => instances += usize::from(instance),
            Err(error) => failure = Some(error),
        }
        count += 1;
    });
    if let Some(failure) = failure {
        return Err(failure);
    }
    assert_eq!(
        count,
        output.len(),
        "the original item results retain physical order"
    );
    assert!(
        instances > 0,
        "selected Generator includes a local weak location witness"
    );
    Ok(count)
}

fn compare_row(
    tree: &RecordTree<'_>,
    ordinal: usize,
    original: &StructValue<'_>,
    locations: &Locations<'_>,
    row: &Json,
    origins: &Origins<'_>,
) -> TestResult<bool> {
    assert_eq!(
        row.pointer("/source/valueIndex"),
        Some(&Json::from(ordinal.to_string())),
        "exact result slot"
    );
    let Some(amount @ Value::Int32(quantity)) = original.field("amount") else {
        return Err("the selected result declares an Int32 quantity".into());
    };
    assert_eq!(
        row.pointer("/amount/value"),
        Some(&Json::from(quantity.to_string())),
        "exact signed item quantity"
    );
    origins.occurrence(
        row.get("amount").ok_or("published amount cell")?,
        tree.record_index(),
        locations.ordinal(amount)?,
        "$value",
    )?;
    check_flags(original, row)?;
    let Some(Value::Reference(target)) = original.field("targetLocation") else {
        return Ok(false);
    };
    let location = row.get("targetLocation").ok_or("published target cell")?;
    assert_eq!(
        location.pointer("/value/reference/targetRecordIndex/value"),
        Some(&Json::from(target.original_target().record_index())),
        "physical target Record index"
    );
    check_target(location, target, tree.root(), locations)
}

fn check_flags(original: &StructValue<'_>, row: &Json) -> TestResult {
    let Some(Value::Array(authored)) = original.field("missionResults") else {
        return Err("the selected result declares an ordered mission outcome array".into());
    };
    let published = row
        .pointer("/missionResults/value")
        .and_then(Json::as_array)
        .ok_or("published outcome flags")?;
    assert_eq!(authored.len(), published.len(), "all outcome flags");
    for (value, cell) in authored.iter().zip(published) {
        let Value::Bool(expected) = value else {
            return Err("the selected original outcome is Bool".into());
        };
        assert_eq!(
            cell.get("value").and_then(Json::as_bool),
            Some(*expected),
            "original outcome value"
        );
    }
    Ok(())
}

fn check_target(
    cell: &Json,
    target: &Reference<'_>,
    root: &StructValue<'_>,
    locations: &Locations<'_>,
) -> TestResult<bool> {
    let original = target.original_target();
    let Some(expected) = target.location() else {
        assert_eq!(
            cell.pointer("/value/reference/target/type")
                .and_then(Json::as_str),
            Some("record"),
            "whole-record weak target"
        );
        return Ok(false);
    };
    let positions = original
        .instance_positions()
        .ok_or("original instance positions")?;
    let ordinal =
        locations.ordinal(at_positions(root, positions).ok_or("original owned target")?)?;
    assert!(
        cell.pointer("/value/reference/target/location/value")
            .and_then(Json::as_str)
            == Some(expected.as_str()),
        "the owned weak target retains its exact original location"
    );
    assert_eq!(
        cell.pointer("/value/targetValue/valueIndex")
            .and_then(Json::as_str),
        Some(ordinal.to_string().as_str()),
        "weak target joins the exact original arena occurrence"
    );
    Ok(true)
}

fn at_positions<'tree, 'source>(
    root: &'tree StructValue<'source>,
    positions: &[u64],
) -> Option<&'tree Value<'source>> {
    let (first, remaining) = positions.split_first()?;
    let mut current = root.fields().get(usize::try_from(*first).ok()?)?.value();
    for position in remaining {
        let index = usize::try_from(*position).ok()?;
        current = match current {
            Value::Struct(owner) => owner.fields().get(index)?.value(),
            Value::Array(items) => items.get(index)?,
            _ => return None,
        };
    }
    Some(current)
}
