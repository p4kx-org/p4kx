// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestContainer, TestEntry};

use crate::Kind;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_inventories::ItemList;
use crate::shop_prefabs::Prefabs;

use super::inventory_source::{items, request};
use super::{CONTAINER, ENTITY, REGISTRY, TestResult, opened, read_shops, registry};

#[test]
fn partial_inventory_json_retains_valid_ids_and_reports_one_failed_entry() -> TestResult {
    let item = "70a98c94-9252-4fd3-a673-5a584339e273";
    let json = format!(r#"{{"Inventory":[{{"ID":{{"ID":["{item}",5,""]}}}},{{"Other":true}}]}}"#);
    let (_folder, archive) = opened(vec![
        TestEntry::stored("padding.txt", "ignored"),
        TestEntry::stored("Data/Scripts/ShopInventories/selected.json", json),
        TestEntry::stored(
            "Data/Scripts/ShopInventories/unused.json",
            "bad unused JSON",
        ),
        request("selected"),
    ])?;
    let mut ledger = ExportLedger::default();
    let items = items(&archive, "selected", &mut ledger)?;
    assert_eq!(
        ledger.incomplete_inventories,
        Vec::new(),
        "fully enumerated JSON values do not hide additional Archive inputs"
    );
    assert_eq!(
        items.test_items(),
        Some((vec![item], 3)),
        "valid IDs survive malformed populated values"
    );
    let [source] = ledger.entries.as_slice() else {
        return Err("one actual inventory operation".into());
    };
    assert_eq!(
        source.entry_index,
        Some(1),
        "the unused corrupt inventory was not read"
    );
    assert_eq!(
        source.operation,
        EntryOperation::ShopInventory,
        "inventory-specific operation"
    );
    assert_eq!(
        source.disposition,
        SourceDisposition::ProjectionFailed,
        "retained partial rows do not certify a complete projection"
    );
    assert_eq!(
        source.diagnosis.accounting_field(source.path.as_str()),
        Some("Inventory[].ID.ID[]"),
        "the technical field survives"
    );
    Ok(())
}

#[test]
fn a_configured_missing_inventory_has_a_logical_identity_and_no_fake_index() -> TestResult {
    let (_folder, archive) = opened(vec![request("missing")])?;
    let mut ledger = ExportLedger::default();
    assert_eq!(
        items(&archive, "missing", &mut ledger)?,
        ItemList::Missing,
        "configured missing file remains distinguishable"
    );
    let [source] = ledger.entries.as_slice() else {
        return Err("one configured missing source".into());
    };
    assert_eq!(source.entry_index, None, "absence has no physical index");
    assert!(
        source.path.ends_with("\\missing.json"),
        "the configured canonical source path survives"
    );
    assert_eq!(
        source.disposition,
        SourceDisposition::ReadFailed,
        "a configured dependency is required"
    );
    assert_eq!(
        source.diagnosis.code,
        Some(ExportDiagnosticCode::DanglingReference),
        "missing configured dependency is explicit"
    );
    Ok(())
}

#[test]
fn an_authored_empty_inventory_is_a_successful_zero_row_projection() -> TestResult {
    let (_folder, archive) = opened(vec![
        TestEntry::stored(
            "Data/Scripts/ShopInventories/empty.json",
            r#"{"Inventory":[]}"#,
        ),
        request("empty"),
    ])?;
    let mut ledger = ExportLedger::default();
    assert_eq!(
        items(&archive, "empty", &mut ledger)?.test_items(),
        Some((Vec::new(), 0)),
        "authored emptiness is retained"
    );
    let [source] = ledger.entries.as_slice() else {
        return Err("one empty inventory operation".into());
    };
    assert_eq!(
        source.disposition,
        SourceDisposition::Projected,
        "zero output rows do not imply failure"
    );
    Ok(())
}

#[test]
fn malformed_prefab_values_retain_valid_entity_inventory_pairs() -> TestResult {
    let xml = r#"<PrefabLibrary><Object Type="Entity" Id="{9730AC77-4077-4DCE-B700-273B6B917450}">
        <PropertiesDataCore><EntityComponentShop offlineInventoryJSON="Scripts/ShopInventories/good.json"
        offlineInventoryXMLName="Scripts/ShopInventories/unsupported.csv"/></PropertiesDataCore>
        </Object></PrefabLibrary>"#;
    let (_folder, archive) = opened(vec![TestEntry::stored("Data/Prefabs/Shops/mixed.xml", xml)])?;
    let prefabs = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        Prefabs::read(&archive, None, capture)
    })?;
    assert_eq!(
        prefabs.ledger.incomplete_inventories,
        Vec::new(),
        "all prefab input attributes were enumerated"
    );
    assert_eq!(
        prefabs.by_entity.get(ENTITY).map(|stems| stems
            .iter()
            .map(crate::shop_inventory_path::InventoryStem::as_str)
            .collect::<Vec<_>>()),
        Some(vec!["good"]),
        "valid inventory pair survives"
    );
    let [source] = prefabs.ledger.entries.as_slice() else {
        return Err("one actual prefab operation".into());
    };
    assert_eq!(
        source.operation,
        EntryOperation::ShopPrefab,
        "source operation is explicit"
    );
    assert_eq!(
        source.disposition,
        SourceDisposition::ProjectionFailed,
        "a populated unsupported field is not reported complete"
    );
    assert_eq!(
        source.diagnosis.code,
        Some(ExportDiagnosticCode::MalformedSource),
        "technical field loss remains visible"
    );
    Ok(())
}

#[test]
fn merged_prefab_inventories_keep_ordered_authored_candidates() -> TestResult {
    let metadata = format!(r#"<Entities><Entity CryGUID="{ENTITY}"/></Entities>"#);
    let soc = format!(
        "CrChF\0{ENTITY}\0EntityComponentShop\0offlineInventoryJSON\0Scripts/ShopInventories/shared.json\0"
    );
    let container = TestContainer::new()
        .entry(TestEntry::stored(
            "test/metadata/entitycomponentshop.xml",
            metadata,
        ))
        .entry(TestEntry::stored("good.soc", soc));
    let prefab = r#"<PrefabLibrary><Object Type="Entity" Id="{9730AC77-4077-4DCE-B700-273B6B917450}">
        <PropertiesDataCore><EntityComponentShop offlineInventoryJSON="Scripts/ShopInventories/SHARED.json"
        offlineInventoryXMLName="Scripts/ShopInventories/Shared.xml"
        offlineInventoryOther="Scripts/ShopInventories/rejected.csv"/></PropertiesDataCore>
        </Object></PrefabLibrary>"#;
    let (_folder, archive) = opened(vec![
        TestEntry::stored(REGISTRY, registry("")),
        TestEntry::stored(CONTAINER, container.build()),
        TestEntry::stored("Data/Prefabs/Shops/mixed.xml", prefab),
        TestEntry::stored(
            "Data/Scripts/ShopInventories/shared.json",
            r#"{"Inventory":[]}"#,
        ),
    ])?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        read_shops(&archive, capture)
    })?;
    assert_eq!(
        (
            read.summary.prefab_entities,
            read.summary.prefab_ambiguous,
            read.summary.prefab_only_shops
        ),
        (1, 0, 0),
        "container and prefab name the same inventory stem"
    );
    let [resolved] = read.resolutions.rows.as_slice() else {
        return Err("one shop inventory resolution".into());
    };
    let row = serde_json::to_value(resolved)?;
    verify_merged_candidates(&row)?;
    let prefab = read
        .ledger
        .entries
        .iter()
        .find(|entry| entry.operation == EntryOperation::ShopPrefab)
        .ok_or("original prefab operation")?;
    assert_eq!(
        (prefab.entry_index, prefab.disposition),
        (Some(2), SourceDisposition::ProjectionFailed),
        "the rejected path retains the original prefab failure"
    );
    Ok(())
}

fn verify_merged_candidates(row: &serde_json::Value) -> TestResult {
    assert_eq!(
        (
            row.pointer("/status/value")
                .and_then(serde_json::Value::as_str),
            row.pointer("/candidates/sourceSetComplete")
                .and_then(serde_json::Value::as_bool),
        ),
        (Some("resolved"), Some(false)),
        "the common stem resolves with the rejected authored path still represented"
    );
    let candidates = row["candidates"]["value"]
        .as_array()
        .ok_or("candidate values")?;
    let authored: Vec<_> = candidates
        .iter()
        .map(|candidate| {
            (
                candidate["value"]["type"].as_str(),
                candidate["value"]["sourcePath"]["value"].as_str(),
            )
        })
        .collect();
    assert_eq!(
        authored,
        [
            (Some("file"), Some("Scripts/ShopInventories/shared.json")),
            (Some("file"), Some("Scripts/ShopInventories/SHARED.json")),
            (Some("file"), Some("Scripts/ShopInventories/Shared.xml")),
            (
                Some("rejected"),
                Some("Scripts/ShopInventories/rejected.csv")
            ),
        ],
        "a common stem keeps each original container and prefab candidate in source order"
    );
    for pair in candidates.windows(2) {
        if let [first, second] = pair {
            assert_ne!(
                first["value"]["sourcePath"]["sourceUsesRefs"],
                second["value"]["sourcePath"]["sourceUsesRefs"],
                "each authored candidate keeps its own original occurrence"
            );
        }
    }
    Ok(())
}

#[test]
fn unreadable_prefab_xml_marks_its_inventory_dependency_walk_incomplete() -> TestResult {
    let (_folder, archive) = opened(vec![TestEntry::stored(
        "Data/Prefabs/Shops/bad.xml",
        "broken XML",
    )])?;
    let prefabs = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        Prefabs::read(&archive, None, capture)
    })?;
    assert_eq!(
        prefabs.ledger.incomplete_inventories,
        [Kind::Shops],
        "unparsed prefab content can conceal configured inventory files"
    );
    let [source] = prefabs.ledger.entries.as_slice() else {
        return Err("one actual failed prefab operation".into());
    };
    assert_eq!(
        source.disposition,
        SourceDisposition::ProjectionFailed,
        "the source bytes were readable"
    );
    Ok(())
}

#[path = "shop_inventory_positional_tests.rs"]
mod positional;
