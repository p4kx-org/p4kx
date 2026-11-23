// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::StarmapTraversalBudget;
use p4kx_export::catalog_queries::shops::{
    ShopRegistrySelection, ShopRowResult, read_shop, read_shops,
};
use p4kx_extraction::{Archive, cryxml, find_entry};
use serde_json::{Value, json};

use super::TestResult;
use crate::health_origin_assertions as health;
use crate::shops_fixture::{self, REGISTRY};

#[test]
fn physical_shop_batch_preserves_order_coverage_and_original_uses() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = crate::shop_items_fixture::write_archive(directory.path())?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let [first, second, duplicate] = physical_registry_nodes(&archive)?;
    let missing = ShopRegistrySelection::new(first.entry_index, u32::MAX);
    let selections = [first, duplicate, second, missing, first];
    let budget = StarmapTraversalBudget::default();
    let batch = read_shops(&archive, &selections, budget, None)?;
    assert_eq!(
        batch.name_surveys, 1,
        "all located rows use one physical name and placement survey"
    );
    assert_eq!(
        batch.rows.len(),
        selections.len(),
        "the request defines output cardinality"
    );
    let provenance = json!({"provenance": batch.source_inventory});
    for (row, (selection, expected_reads)) in batch
        .rows
        .iter()
        .zip(selections.into_iter().zip([1, 0, 0, 0, 1]))
    {
        assert_position(&archive, row, selection, expected_reads, &provenance)?;
        if selection == duplicate || selection == missing {
            assert!(
                row.row.is_none(),
                "duplicate and missing original positions stay empty"
            );
        } else {
            assert!(
                row.row
                    .as_ref()
                    .is_some_and(|shop| shop.starmap_object_guid.value().is_some()),
                "the fixture demands actual location matches"
            );
        }
    }
    health::closure(
        &json!({"provenance": provenance.get("provenance"), "shops": batch.rows.iter().map(|row| &row.row).collect::<Vec<_>>() }),
    )?;
    let negatives = read_shops(&archive, &[duplicate, missing], budget, None)?;
    assert_eq!(
        negatives.name_surveys, 0,
        "the absent-only batch opens no name survey"
    );
    Ok(())
}

pub(super) fn physical_registry_nodes(
    archive: &Archive,
) -> Result<[ShopRegistrySelection; 3], Box<dyn std::error::Error>> {
    let entry = find_entry(archive, REGISTRY).ok_or("the original registry Entry")?;
    let registry = shops_fixture::registry();
    let document = cryxml::parse(&registry)?;
    let nodes: Vec<_> = document
        .root()
        .children()
        .flat_map(cryxml::Element::children)
        .flat_map(cryxml::Element::children)
        .flat_map(cryxml::Element::children)
        .filter(|node| node.name() == "ShopBackup")
        .collect();
    let entry_index = u32::try_from(entry.index())?;
    let select = |name| {
        nodes
            .iter()
            .find(|node| node.attribute("ShopName") == Some(name))
            .map(|node| ShopRegistrySelection::new(entry_index, node.index()))
            .ok_or("the fixture contains the physical ShopBackup node")
    };
    Ok([
        select("L_TDD_Hurston")?,
        select("Clothing_A")?,
        select("Duplicate")?,
    ])
}

fn assert_position(
    archive: &Archive,
    row: &ShopRowResult,
    selection: ShopRegistrySelection,
    expected_reads: usize,
    provenance: &Value,
) -> TestResult {
    let cold = read_shop(archive, selection, StarmapTraversalBudget::default(), None)?;
    assert_eq!(
        row.selection, selection,
        "the result keeps its exact request position"
    );
    assert_eq!(
        row.coverage, cold.coverage,
        "each position keeps the same physical coverage"
    );
    assert_eq!(
        serde_json::to_value(row.availability)?,
        serde_json::to_value(cold.availability)?,
        "registry availability stays independent of row admission"
    );
    assert_eq!(
        row.coverage.inventory_payloads, expected_reads,
        "only admitted selected payloads count"
    );
    let actual = health::with_origins(serde_json::to_value(&row.row)?, provenance)?;
    let expected = health::with_origins(
        serde_json::to_value(cold.row)?,
        &json!({"provenance": cold.source_inventory}),
    )?;
    assert_eq!(
        actual, expected,
        "the shared capture retains each row's exact contributing uses"
    );
    Ok(())
}
