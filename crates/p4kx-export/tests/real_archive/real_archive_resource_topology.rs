// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::catalog_v3_consumer;

use crate::real_archive_item_components as components;
use crate::real_archive_item_durability as proof;

#[path = "real_archive_resource_topology_sample.rs"]
mod sample;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn avenger_and_multi_relay_topology_match_the_authored_source() -> TestResult {
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
        .map_err(|_| "build resource topology catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let mut avenger = None;
    for row in proof::rows(&catalog, "/ships")? {
        let Some(name) = catalog_v3_consumer::source_value(&row["className"])? else {
            continue;
        };
        if name
            .as_str()
            .is_some_and(|name| name.eq_ignore_ascii_case("AEGS_Avenger_Titan"))
        {
            avenger = Some(row);
            break;
        }
    }
    let avenger = avenger.ok_or("configured build contains no readable target Avenger Record")?;
    let avenger = u32::try_from(proof::number(avenger, "/recordIndex/value")?)?;
    let multi = multi_relay(&catalog, avenger)?;
    for record in [avenger, multi] {
        trace(&forge, &catalog, record, Declaration::Hardpoint)?;
        trace(&forge, &catalog, record, Declaration::Room)?;
    }
    watch
        .check()
        .map_err(|_| "Archive changed during topology verification")?;
    crate::real_archive_verification::print_line(
        "resource topology: two independent ship scopes matched authored links and room selectors",
    );
    Ok(())
}

fn multi_relay(catalog: &Value, avenger: u32) -> TestResult<u32> {
    let mut loadouts = BTreeSet::new();
    for row in proof::rows(catalog, "/loadouts")? {
        let index = u32::try_from(proof::number(row, "/sourceRecordIndex/value")?)?;
        assert!(
            loadouts.insert(index),
            "published Loadout Records are unique"
        );
    }
    let mut relay_counts = BTreeMap::new();
    for node in proof::rows(catalog, "/placements/resourceNetworkNodes/rows")? {
        let index = u32::try_from(proof::number(node, "/position/loadoutRecordIndex")?)?;
        assert!(
            loadouts.contains(&index),
            "physical node Loadout is published"
        );
        let evidence = proof::rows(node, "/resourceComponents")?;
        if evidence
            .iter()
            .any(|source| source.pointer("/isRelay/value") == Some(&Value::Bool(true)))
        {
            *relay_counts.entry(index).or_insert(0) += 1;
        }
    }
    for (index, count) in relay_counts {
        if index != avenger && count >= 2 {
            return Ok(index);
        }
    }
    Err("configured build contains no independent multi-relay ship scope".into())
}

#[derive(Clone, Copy)]
enum Declaration {
    Hardpoint,
    Room,
}

impl Declaration {
    const fn fields(self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self {
            Self::Hardpoint => (
                "/declarations/resourceNetworkLinks/rows",
                "SItemPortContainerComponentParams",
                "InternalHardpointLinks",
                "internalHardpointLinks",
            ),
            Self::Room => (
                "/declarations/roomAssociations/rows",
                "VehicleComponentParams",
                "objectContainers",
                "objectContainers",
            ),
        }
    }

    const fn members(self) -> &'static [&'static str] {
        match self {
            Self::Hardpoint => &["port1", "port2"],
            Self::Room => &[
                "resourceNetworkItemportName",
                "boneName",
                "fileName",
                "guid",
            ],
        }
    }
}

fn trace(
    forge: &DataForge<'_>,
    catalog: &Value,
    record_index: u32,
    declaration: Declaration,
) -> TestResult {
    let (table, type_name, source_field, published_field) = declaration.fields();
    let mut trees = TreeBuilder::new();
    let mut visited = 0;
    for ((owner, ordinal), row) in components::component_rows(catalog, table)? {
        if owner != record_index {
            continue;
        }
        let record = forge.record(owner).ok_or("original topology Record")?;
        let tree = trees.build(&record).map_err(|_| "read topology Record")?;
        let Some(Original::Array(parts)) = tree.field("Components") else {
            return Err("topology Record Components".into());
        };
        let Some(Original::Struct(source)) = parts.get(ordinal) else {
            return Err("original topology Components slot".into());
        };
        if source.type_name() != type_name
            || proof::text(row, "/ownerGuid/value")? != tree.guid().to_string()
        {
            return Err("original topology owner differs".into());
        }
        let cell = components::cell(row, published_field)?;
        compare_slots(source.field(source_field), cell, declaration.members())?;
        visited += 1;
    }
    if visited == 0 {
        return Err("no physical source owner found for the representative ship scope".into());
    }
    Ok(())
}

fn compare_slots(source: Option<&Original<'_>>, cell: &Value, fields: &[&str]) -> TestResult {
    let Some(Original::Array(originals)) = source else {
        return components::require_state(
            cell,
            if source.is_none() {
                "absent"
            } else {
                "unreadable"
            },
        );
    };
    components::require_state(cell, "present")?;
    let rows = proof::rows(cell, "/value")?;
    if originals.len() != rows.len() {
        return Err("original topology occurrence count differs".into());
    }
    for (original, row) in originals.iter().zip(rows) {
        let Original::Struct(original) = original else {
            return Err("unreadable topology occurrence in representative source".into());
        };
        for field in fields {
            let original = original.field(field);
            let rendered =
                components::cell(row.get("value").ok_or("authored occurrence value")?, field)?;
            match original {
                Some(Original::String(expected)) => {
                    if proof::text(rendered, "/value")? != *expected {
                        return Err("authored topology word differs".into());
                    }
                }
                None => components::require_state(rendered, "absent")?,
                _ => return Err("unexpected original topology String shape".into()),
            }
        }
    }
    Ok(())
}
