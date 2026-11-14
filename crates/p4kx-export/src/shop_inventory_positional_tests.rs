// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::TestEntry;

use crate::Kind;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_inventories::ItemList;

use super::{TestResult, items, opened, request};

const ITEM: &str = "70a98c94-9252-4fd3-a673-5a584339e273";
const INVENTORY: &str = "Data/Scripts/ShopInventories/selected.json";

#[test]
fn legacy_inventory_structs_accept_positional_json() -> TestResult {
    let cases = [
        ("file", format!(r#"[null,[{{"ID":{{"ID":["{ITEM}"]}}}}]]"#)),
        (
            "collection",
            format!(r#"{{"Collection":[[{{"ID":{{"ID":["{ITEM}"]}}}}]]}}"#),
        ),
        (
            "identified line",
            format!(r#"{{"Inventory":[[{{"ID":["{ITEM}"]}}]]}}"#),
        ),
        (
            "ID container",
            format!(r#"{{"Inventory":[{{"ID":[["{ITEM}"]]}}]}}"#),
        ),
    ];
    for (shape, body) in cases {
        let (list, ledger) = selected(&body)?;
        assert_eq!(
            list,
            ItemList::Items {
                guids: vec![ITEM.to_owned()],
                dropped: 0,
            },
            "the original positional {shape} supplies the selected item"
        );
        let [source] = ledger.entries.as_slice() else {
            return Err(format!("expected one selected source for {shape}").into());
        };
        assert_eq!(
            source.entry_index,
            Some(0),
            "physical inventory for {shape}"
        );
        assert_eq!(
            source.kind,
            Kind::Shops,
            "selected Shop dependency for {shape}"
        );
        assert_eq!(
            source.disposition,
            SourceDisposition::Projected,
            "a valid positional {shape} is a complete selected projection"
        );
    }
    Ok(())
}

#[test]
fn duplicate_file_members_refuse_the_list_but_duplicate_item_members_drop_one_line() -> TestResult {
    let (list, ledger) = selected(r#"{"Inventory":[],"Inventory":[]}"#)?;
    check_malformed(list, &ledger)?;
    let body = format!(
        r#"{{"Inventory":[{{"ID":{{"ID":["{ITEM}"],"ID":["{ITEM}"]}}}},{{"ID":{{"ID":["{ITEM}"]}}}}]}}"#
    );
    let (list, ledger) = selected(&body)?;
    assert_eq!(
        list,
        ItemList::Items {
            guids: vec![ITEM.to_owned()],
            dropped: 1,
        },
        "the line with a duplicate ID is dropped and the valid line survives"
    );
    let [source] = ledger.entries.as_slice() else {
        return Err("one selected inventory source".into());
    };
    assert_eq!(
        source.diagnosis.accounting_field(source.path.as_str()),
        Some("Inventory[].ID.ID[]"),
        "an invalid item retains the original item diagnosis"
    );
    Ok(())
}

#[test]
fn syntactically_malformed_inventory_keeps_the_selected_dependency_diagnosis() -> TestResult {
    let (list, ledger) = selected(r#"{"Inventory":["#)?;
    check_malformed(list, &ledger)
}

fn selected(body: &str) -> Result<(ItemList, ExportLedger), Box<dyn Error>> {
    let (_folder, archive) = opened(vec![
        TestEntry::stored(INVENTORY, body),
        request("selected"),
    ])?;
    let mut ledger = ExportLedger::default();
    let list = items(&archive, "selected", &mut ledger)?;
    Ok((list, ledger))
}

fn check_malformed(list: ItemList, ledger: &ExportLedger) -> TestResult {
    let ItemList::Unreadable(reason) = list else {
        return Err("the selected inventory must be unreadable".into());
    };
    assert!(
        reason.ends_with(": the file is no inventory JSON"),
        "the original selected-inventory reason survives: {reason}"
    );
    let [source] = ledger.entries.as_slice() else {
        return Err("one selected inventory source".into());
    };
    assert_eq!(
        source.entry_index,
        Some(0),
        "the actual unreadable inventory"
    );
    assert_eq!(source.kind, Kind::Shops, "selected Shop dependency");
    assert_eq!(
        source.disposition,
        SourceDisposition::ProjectionFailed,
        "reading the Entry succeeded before the interpretation failed"
    );
    assert_eq!(
        source.diagnosis.code,
        Some(ExportDiagnosticCode::MalformedSource),
        "the selected malformed interpretation remains explicit"
    );
    assert_eq!(
        source.diagnosis.accounting_field(source.path.as_str()),
        Some("Collection.Inventory"),
        "the existing selected-dependency field survives"
    );
    Ok(())
}
