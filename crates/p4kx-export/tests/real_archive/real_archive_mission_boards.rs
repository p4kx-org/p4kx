// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, StructValue, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::authored_definition_witness::{field, verify_array, verify_link, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_verification::print_line;

const OWNER: &str = "EntityClassDefinition";
const COMPONENT: &str = "EntityComponentMissionBoardUIProvider";
const REPUTATION: &str = "FactionReputation";

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn mission_board_providers_match_original_component_slots() -> TestResult {
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
        .map_err(|_| "build mission board catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let rows = catalog
        .pointer("/configurations/missionBoards/rows")
        .and_then(Value::as_array)
        .ok_or("mission board provider rows")?;
    verify_rows(&forge, rows)?;
    watch
        .check()
        .map_err(|_| "Archive changed during board verification")?;
    print_line(&format!(
        "mission boards: {} original provider slots checked",
        rows.len()
    ));
    Ok(())
}

fn verify_rows(forge: &DataForge<'_>, rows: &[Value]) -> TestResult {
    let Some(probe) = forge.pointer_probe("Components", COMPONENT) else {
        assert!(rows.is_empty(), "no board provider instances");
        return Ok(());
    };
    let mut emitted = rows.iter();
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        if record.type_name().ok() != Some(OWNER) || !trees.holds(&probe, &record)? {
            continue;
        }
        let tree = trees
            .build(&record)
            .map_err(|_| "read original board owner")?;
        let Some(Original::Array(components)) = tree.field("Components") else {
            return Err("original Components array".into());
        };
        for (ordinal, component) in components.iter().enumerate() {
            let Original::Struct(source) = component else {
                continue;
            };
            if source.type_name() != COMPONENT {
                continue;
            }
            let row = emitted.next().ok_or("missing board provider row")?;
            let owner = field(row, "owner")?;
            let guid = consumer::present(field(owner, "ownerGuid")?)?.as_str();
            assert_eq!(
                guid,
                Some(record.guid().to_string().as_str()),
                "physical owner GUID"
            );
            let index = consumer::present(field(owner, "ownerRecordIndex")?)?.as_u64();
            assert_eq!(
                index,
                Some(u64::from(record.index())),
                "physical Record index"
            );
            let slot = consumer::present(field(owner, "componentOrdinal")?)?.as_u64();
            assert_eq!(
                slot,
                Some(u64::try_from(ordinal)?),
                "original component position"
            );
            verify_board(forge, source, row)?;
        }
    }
    assert!(emitted.next().is_none(), "no extra provider rows");
    Ok(())
}

fn verify_board(forge: &DataForge<'_>, component: &StructValue<'_>, row: &Value) -> TestResult {
    let original = component.field("boardType");
    let board = field(row, "boardType")?;
    let Some(Original::Struct(original)) = original else {
        return verify_state(original, board);
    };
    let value = consumer::present(board)?;
    let source_type = consumer::present(field(value, "sourceType")?)?.as_str();
    assert_eq!(
        source_type,
        Some(original.type_name()),
        "original board type"
    );
    let details = field(value, "details")?;
    let expected = match original.type_name() {
        "MissionBoardType_Base" => "base",
        "MissionBoardType_Career" => "career",
        "MissionBoardType_Notice" => "notice",
        _ => "authored",
    };
    assert_eq!(
        field(details, "type")?.as_str(),
        Some(expected),
        "observed alternative"
    );
    if expected == "career" {
        verify_link(
            forge,
            REPUTATION,
            original.field("guildFactionReputation"),
            field(details, "guildFactionReputation")?,
        )?;
        verify_array(
            original.field("factions"),
            field(details, "factions")?,
            |slot, row| verify_link(forge, REPUTATION, slot, row),
        )?;
    }
    Ok(())
}
