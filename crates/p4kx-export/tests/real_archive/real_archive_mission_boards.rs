// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, RecordTree, StructValue, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::authored_definition_witness::{field, verify_array, verify_link, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_auto_spawns::original::Locations;
use crate::real_archive_auto_spawns::provenance::Origins;

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
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let selected = selected_owners(&forge, &watch)?;
    if selected.is_empty() {
        if std::env::var_os("P4KX_EXPECT_MISSION_BOARD_PROVIDERS").is_some() {
            return Err("configured Archive lacks authored mission board components".into());
        }
        crate::real_archive_verification::print_line("no source-selected mission board providers");
    }
    for index in selected {
        let record = forge.record(index).ok_or("original board owner")?;
        let tree = TreeBuilder::new()
            .build(&record)
            .map_err(|_| "read original board owner")?;
        compare_owner(&archive, &entry, &forge, &tree)?;
        watch
            .check()
            .map_err(|_| "Archive changed during board verification")?;
    }
    Ok(())
}

fn selected_owners(forge: &DataForge<'_>, watch: &ArchiveWatch) -> TestResult<BTreeSet<u32>> {
    let Some(probe) = forge.pointer_probe("Components", COMPONENT) else {
        return Ok(BTreeSet::new());
    };
    let mut types = BTreeMap::new();
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        watch.poll().map_err(|_| "Archive changed")?;
        if record.type_name().ok() != Some(OWNER)
            || record
                .record_path()
                .map_err(|_| "original board owner path")?
                .contains(['*', '?'])
            || !trees
                .holds(&probe, &record)
                .map_err(|_| "inspect board provider ownership")?
        {
            continue;
        }
        let tree = trees
            .build(&record)
            .map_err(|_| "read original board owner")?;
        for (_, original, _) in components(&tree)? {
            let variant = match original.field("boardType") {
                Some(Original::Struct(board)) => board.type_name(),
                Some(Original::Null) => "null",
                Some(Original::Error(_)) => "unreadable",
                _ => "other",
            };
            types.entry(variant).or_insert_with(|| record.index());
        }
    }
    Ok(types.into_values().collect())
}

fn components<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> TestResult<impl Iterator<Item = (usize, &'tree StructValue<'source>, &'tree Original<'source>)>>
{
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original Components array".into());
    };
    Ok(components
        .iter()
        .enumerate()
        .filter_map(|(ordinal, component)| match component {
            Original::Struct(source) if source.type_name() == COMPONENT => {
                Some((ordinal, source, component))
            }
            _ => None,
        }))
}

fn compare_owner(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    tree: &RecordTree<'_>,
) -> TestResult {
    let filter = Filter::new(tree.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built =
        build_catalog_from(archive, entry, &options).map_err(|_| "build selected board catalog")?;
    let catalog: Value = serde_json::from_slice(built.json()).map_err(|_| "read catalog JSON")?;
    crate::real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    let origins = Origins::read(&catalog, entry.origin())?;
    let rows = catalog
        .pointer("/configurations/missionBoards/rows")
        .and_then(Value::as_array)
        .ok_or("mission board provider rows")?;
    let locations = Locations::read(tree.root());
    let mut rows = rows.iter().filter(|row| {
        row.pointer("/owner/ownerRecordIndex/value")
            .and_then(Value::as_u64)
            == Some(u64::from(tree.record_index()))
    });
    let guid = tree.guid().to_string();
    for (ordinal, original, component) in components(tree)? {
        let row = rows.next().ok_or("missing board provider row")?;
        let owner = field(row, "owner")?;
        assert!(
            consumer::present(field(owner, "ownerGuid")?)?.as_str() == Some(guid.as_str())
                && consumer::present(field(owner, "componentOrdinal")?)?.as_u64()
                    == Some(u64::try_from(ordinal)?),
            "the original board provider remains attached to its physical owner and slot"
        );
        origins.occurrence(
            field(owner, "componentOrdinal")?,
            tree.record_index(),
            locations.ordinal(component)?,
            "$value",
        )?;
        verify_board(forge, original, row)?;
        if let Some(source @ Original::Struct(_)) = original.field("boardType") {
            for cell in [
                field(row, "boardType")?,
                field(consumer::present(field(row, "boardType")?)?, "sourceType")?,
            ] {
                origins.occurrence(
                    cell,
                    tree.record_index(),
                    locations.ordinal(source)?,
                    "$type",
                )?;
            }
        }
    }
    assert!(
        rows.next().is_none(),
        "no extra board provider slots for this owner"
    );
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
