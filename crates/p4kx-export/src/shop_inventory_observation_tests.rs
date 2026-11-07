// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::TestEntry;

use crate::Kind;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_inventories::{Inventories, ItemList};
use crate::shop_prefabs::Prefabs;

use super::{ENTITY, TestResult, opened};

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
    ])?;
    let inventories = Inventories::new(&archive);
    let mut ledger = ExportLedger::default();
    let items = inventories.items("selected", &mut ledger);
    assert_eq!(
        ledger.incomplete_inventories,
        Vec::new(),
        "fully enumerated JSON values do not hide additional Archive inputs"
    );
    assert_eq!(
        items,
        ItemList::Items {
            guids: vec![item.to_owned()],
            dropped: 3
        },
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
        source.diagnosis.field.as_deref(),
        Some("Inventory[].ID.ID[]"),
        "the technical field survives"
    );
    Ok(())
}

#[test]
fn a_configured_missing_inventory_has_a_logical_identity_and_no_fake_index() -> TestResult {
    let (_folder, archive) = opened(Vec::new())?;
    let mut ledger = ExportLedger::default();
    assert_eq!(
        Inventories::new(&archive).items("missing", &mut ledger),
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
    let (_folder, archive) = opened(vec![TestEntry::stored(
        "Data/Scripts/ShopInventories/empty.json",
        r#"{"Inventory":[]}"#,
    )])?;
    let mut ledger = ExportLedger::default();
    assert_eq!(
        Inventories::new(&archive).items("empty", &mut ledger),
        ItemList::Items {
            guids: Vec::new(),
            dropped: 0
        },
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
    let prefabs = Prefabs::read(&archive, None);
    assert_eq!(
        prefabs.ledger.incomplete_inventories,
        Vec::new(),
        "all prefab input attributes were enumerated"
    );
    assert_eq!(
        prefabs.by_entity.get(ENTITY),
        Some(&vec!["good".to_owned()]),
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
fn unreadable_prefab_xml_marks_its_inventory_dependency_walk_incomplete() -> TestResult {
    let (_folder, archive) = opened(vec![TestEntry::stored(
        "Data/Prefabs/Shops/bad.xml",
        "broken XML",
    )])?;
    let prefabs = Prefabs::read(&archive, None);
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
