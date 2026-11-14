// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, RecordTree, StructValue, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_pair};
use serde_json::Value;

use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};
use crate::real_archive_item_components::{cell, require_state};
use crate::real_archive_shop_inventory_v3;
use crate::real_archive_verification;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const OPTIONS: &str = "radarOptionParams";
const PING: &str = "allowFPSPingWave";
const FLAGS: [&str; 7] = [
    "useFPSTagging",
    "useFPSScanMode",
    "enableScanMode",
    "useExperimentalFeatures",
    "allowDeltaSignature",
    "allowEmissionsHUD",
    PING,
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn game_mode_radar_options_match_original_value_and_origin_in_both_builds() -> TestResult {
    let Some((historical, current)) = real_archive_pair()? else {
        return Ok(());
    };
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    for (sample, path, expected_ping) in [(1, historical, false), (2, current, true)] {
        let archive = Archive::open_with_key(path, Some(key.clone()))
            .map_err(|_| "open configured Archive")?;
        let watch = ArchiveWatch::new(&archive);
        let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
        let forge = entry.open().map_err(|_| "open original DataForge")?;
        let owner = select_mode(&forge, &watch, expected_ping)?;
        let filter = Filter::new(owner.record_path());
        let options = CatalogOptions {
            filter: Some(&filter),
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..CatalogOptions::new(UNIX_EPOCH)
        };
        let built = build_catalog_from(&archive, &entry, &options)
            .map_err(|_| "build owner-filtered radar Catalog")?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
        verify(&catalog, &owner, &Origins::read(&catalog, entry.origin())?)?;
        watch
            .check()
            .map_err(|_| "Archive changed during verification")?;
        real_archive_verification::print_line(&format!(
            "radar options witness {sample}: original physical owner and declared Boolean source states verified"
        ));
    }
    Ok(())
}

fn select_mode<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
    expected_ping: bool,
) -> TestResult<RecordTree<'source>> {
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        if !matches!(record.type_name(), Ok("GameMode")) {
            continue;
        }
        watch
            .poll()
            .map_err(|_| "Archive changed during source selection")?;
        let tree = trees
            .build(&record)
            .map_err(|_| "read original GameMode Record")?;
        if let Some((_, Original::Struct(radar))) = radar(&tree)
            && (matches!(radar.field(PING), Some(Original::Bool(_))) && expected_ping
                || radar.field(PING).is_none() && !expected_ping)
        {
            return Ok(tree);
        }
    }
    Err("no GameMode with the expected declared radar options shape".into())
}

fn radar<'read, 'source>(
    tree: &'read RecordTree<'source>,
) -> Option<(&'read StructValue<'source>, &'read Original<'source>)> {
    let Some(Original::Struct(params)) = tree.field("params") else {
        return None;
    };
    (params.type_name() == "SStandardParams").then_some((params, params.field(OPTIONS)?))
}

fn verify(catalog: &Value, tree: &RecordTree<'_>, origins: &Origins<'_>) -> TestResult {
    let owner = tree.record_index();
    let rows = catalog
        .pointer("/configurations/radarOptions/rows")
        .and_then(Value::as_array)
        .ok_or("typed radar options table")?;
    let [row] = rows.as_slice() else {
        return Err("one physical GameMode in the original path filter".into());
    };
    if row
        .pointer("/source/recordIndex/value")
        .and_then(Value::as_u64)
        != Some(u64::from(owner))
        || row.pointer("/source/guid/value").and_then(Value::as_str)
            != Some(tree.guid().to_string().as_str())
        || row
            .pointer("/source/recordPath/value")
            .and_then(Value::as_str)
            != Some(tree.record_path())
    {
        return Err("radar options owner differs".into());
    }
    let Some((params, original @ Original::Struct(radar))) = radar(tree) else {
        return Err("expected an original inline radar configuration".into());
    };
    let row = row.pointer("/params/value").ok_or("original params body")?;
    assert_eq!(
        row.pointer("/sourceType/value").and_then(Value::as_str),
        Some(params.type_name()),
        "physical concrete mode params"
    );
    let locations = Locations::read(tree.root());
    let carrier = cell(row, OPTIONS)?;
    require_state(carrier, "present")?;
    origins.occurrence(carrier, owner, locations.ordinal(original)?, "$type")?;
    let values = carrier.get("value").ok_or("radar options body")?;
    assert_eq!(
        values.pointer("/sourceType/value").and_then(Value::as_str),
        Some(radar.type_name()),
        "physical concrete radar options type"
    );
    for name in FLAGS {
        let cell = cell(values, name)?;
        match radar.field(name) {
            None => require_state(cell, "absent")?,
            Some(raw @ Original::Bool(flag)) => {
                require_state(cell, "present")?;
                assert!(
                    cell.get("value").and_then(Value::as_bool) == Some(*flag),
                    "authored Boolean survives"
                );
                origins.occurrence(cell, owner, locations.ordinal(raw)?, "$value")?;
            }
            _ => return Err("unexpected original radar Boolean shape".into()),
        }
    }
    Ok(())
}
