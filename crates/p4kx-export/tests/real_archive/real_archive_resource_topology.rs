// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

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
    let avenger = proof::rows(&catalog, "/ships")?
        .iter()
        .find(|row| {
            row["className"]
                .as_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("AEGS_Avenger_Titan"))
        })
        .ok_or("configured build contains no target Avenger Record")?;
    let avenger = proof::text(avenger, "/guid")?;
    let multi = multi_relay(&catalog, avenger)?;
    for guid in [avenger, multi] {
        trace(&forge, &catalog, guid, Declaration::Hardpoint)?;
        trace(&forge, &catalog, guid, Declaration::Room)?;
    }
    watch
        .check()
        .map_err(|_| "Archive changed during topology verification")?;
    crate::real_archive_verification::print_line(
        "resource topology: two independent ship scopes matched authored links and room selectors",
    );
    Ok(())
}

fn multi_relay<'catalog>(catalog: &'catalog Value, avenger: &str) -> TestResult<&'catalog str> {
    let nodes = proof::rows(catalog, "/placements/resourceNetworkNodes/rows")?;
    let loadouts = proof::rows(catalog, "/loadouts")?;
    let mut relay_counts = BTreeMap::new();
    for node in nodes {
        let evidence = proof::rows(node, "/resourceComponents")?;
        if evidence
            .iter()
            .any(|source| source.pointer("/isRelay/value") == Some(&Value::Bool(true)))
        {
            let ordinal = usize::try_from(proof::number(node, "/position/loadoutOrdinal")?)?;
            *relay_counts.entry(ordinal).or_insert(0) += 1;
        }
    }
    for (ordinal, count) in relay_counts {
        let loadout = loadouts.get(ordinal).ok_or("published Loadout ordinal")?;
        let guid = proof::text(loadout, "/entityGuid")?;
        if guid != avenger && count >= 2 {
            return Ok(guid);
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
    guid: &str,
    declaration: Declaration,
) -> TestResult {
    let (table, type_name, source_field, published_field) = declaration.fields();
    let mut trees = TreeBuilder::new();
    let mut visited = 0;
    for ((owner, ordinal), row) in components::component_rows(catalog, table)? {
        if proof::text(row, "/ownerGuid/value")? != guid {
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
        if source.type_name() != type_name || tree.guid().to_string() != guid {
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
