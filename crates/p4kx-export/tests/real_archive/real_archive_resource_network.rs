// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, RecordTree, StructValue, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_components::{self as components, require_state};
use crate::real_archive_item_durability as proof;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const TABLE: &str = "/configurations/resourceNetworkParameters/rows";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn game_mode_resource_rules_trace_actual_owner_and_original_fields() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build resource rules catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let rows = proof::rows(&catalog, TABLE)?;
    let mut modes_with_rules = 0_usize;
    let mut trees = TreeBuilder::new();
    let mut previous = None;
    for row in rows {
        let owner = u32::try_from(proof::number(row, "/source/recordIndex/value")?)?;
        if previous.is_some_and(|earlier| earlier >= owner) {
            return Err("global rule owners must preserve physical order".into());
        }
        previous = Some(owner);
        let record = forge
            .record(owner)
            .ok_or("original game-mode Record index")?;
        let tree = trees
            .build(&record)
            .map_err(|_| "read original game-mode Record")?;
        modes_with_rules += usize::from(compare_mode(&tree, row)?);
    }
    watch
        .check()
        .map_err(|_| "Archive changed during resource rule verification")?;
    if modes_with_rules == 0 {
        return Err("configured build contains no traced global resource rule".into());
    }
    crate::real_archive_verification::print_line(&format!(
        "resource-network rules: {modes_with_rules} original rules across {} GameMode owners",
        rows.len()
    ));
    Ok(())
}

fn compare_mode(tree: &RecordTree<'_>, row: &Value) -> TestResult<bool> {
    if tree.type_name() != "GameMode"
        || proof::text(row, "/source/sourceType/value")? != tree.type_name()
        || proof::text(row, "/source/guid/value")? != tree.guid().to_string()
        || proof::text(row, "/source/recordPath/value")? != tree.record_path()
    {
        return Err("global resource rule owner differs".into());
    }
    let original = tree.field("params");
    let cell = components::cell(row, "params")?;
    let params = match original {
        None => {
            require_state(cell, "absent")?;
            return Ok(false);
        }
        Some(Original::Null) => {
            require_state(cell, "null")?;
            return Ok(false);
        }
        Some(Original::Struct(value)) => value,
        _ => return Err("unreadable original game-mode parameters".into()),
    };
    let output = cell.get("value").ok_or("original game-mode params cell")?;
    if proof::text(output, "/sourceType/value")? != params.type_name() {
        return Err("concrete game-mode parameter type differs".into());
    }
    let rules_cell = components::cell(output, "resourceNetworkParams")?;
    match params.field("resourceNetworkParams") {
        None => {
            require_state(rules_cell, "absent")?;
            Ok(false)
        }
        Some(Original::Null) => {
            require_state(rules_cell, "null")?;
            Ok(false)
        }
        Some(Original::Struct(rules)) => {
            compare_rules(rules, rules_cell.get("value").ok_or("original rules cell")?)?;
            Ok(true)
        }
        _ => Err("unreadable original resource rules".into()),
    }
}

fn compare_rules(source: &StructValue<'_>, row: &Value) -> TestResult {
    if proof::text(row, "/sourceType/value")? != source.type_name() {
        return Err("original resource rules declaration differs".into());
    }
    for field in [
        "enableItemResourceNetwork",
        "enableSalvageItemRepair",
        "enableMisfires",
        "enableCriticalMisfires",
        "doorsIgnoreRN",
        "enableExperimentalArmorPenetration",
        "shipPassDestroyedPartDamageToComponent",
        "shipPassDestroyedPartDamageToChild",
        "shipDestructionBufferMultiplier",
        "shipDestructionHealthThreshold",
    ] {
        let cell = components::cell(row, field)?;
        let value = cell.get("value").ok_or("global rule payload")?;
        let same = match source.field(field) {
            None => {
                require_state(cell, "absent")?;
                continue;
            }
            Some(Original::Bool(expected)) => value.as_bool() == Some(*expected),
            Some(Original::Single(expected)) => {
                value["rawBits"].as_str() == Some(format!("{:08x}", expected.to_bits()).as_str())
            }
            _ => return Err("unreadable original global rule".into()),
        };
        if !same {
            return Err(format!("original global resource setting {field} differs").into());
        }
        require_state(cell, "present")?;
    }
    Ok(())
}
