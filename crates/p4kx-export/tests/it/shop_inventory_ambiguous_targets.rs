// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use serde_json::{Value, json};

use super::{INVENTORIES, ITEM_A, TestResult, key, original, resolution_row, uses};
use crate::health_origin_assertions as health;

const FILES: [&str; 2] = ["Inv_Left.json", "Inv_Right.json"];

pub(super) fn assert_original(archive: &Archive) -> TestResult<[usize; 2]> {
    let mut indices = [0; 2];
    for (index, name) in indices.iter_mut().zip(FILES) {
        let path = format!("{INVENTORIES}{name}");
        let entry = archive
            .entries()
            .iter()
            .find(|entry| entry.path() == path)
            .ok_or("the original ambiguous alternative exists")?;
        let document: Value = serde_json::from_str(&original(archive, &path)?)?;
        assert_eq!(
            document.pointer("/Inventory/0/ID/ID/0"),
            Some(&json!(ITEM_A)),
            "each actual ambiguous alternative has readable inventory content"
        );
        *index = entry.index();
    }
    Ok(indices)
}

pub(super) fn assert_export(catalog: &Value, rows: &[Value], indices: [usize; 2]) -> TestResult {
    let key = key(2);
    let legacy = health::rows(catalog, "/shops")?
        .iter()
        .find(|shop| shop["key"] == key)
        .ok_or("the original ambiguous shop")?;
    assert!(
        legacy["inventory"].is_null() && legacy["itemGuids"].is_null(),
        "conflicting authored stems still produce no legacy selection"
    );

    let row = resolution_row(rows, &key)?;
    let targets = health::rows(row, "/physicalTargets/value")?;
    assert_eq!(
        targets.len(),
        FILES.len(),
        "repeated stems do not repeat physical targets"
    );
    let definitions = health::rows(catalog, "/definitions/shopInventories/rows")?;
    let aggregate_uses = health::rows(&row["physicalTargets"], "/sourceUsesRefs")?;
    let status_uses = health::rows(&row["status"], "/sourceUsesRefs")?;
    for (target, index) in targets.iter().zip(indices) {
        let definition = definitions
            .iter()
            .find(|entry| entry["entryIndex"] == index)
            .ok_or("physical inventory definition for the original alternative")?;
        assert_eq!(
            target["value"],
            json!({"inputRef": definition["inputRef"], "entryIndex": index}),
            "each backed candidate links in first-declaration order to its exact original file"
        );
        let directory_uses = uses(catalog, target)?;
        assert_eq!(
            directory_uses.len(),
            2,
            "the exact directory path and index selectors"
        );
        assert!(
            directory_uses.iter().all(|used| {
                used["role"] == "selector"
                    && aggregate_uses.contains(&used["id"])
                    && !status_uses.contains(&used["id"])
            }),
            "target enumeration retains actual directory evidence while declaration conflict determines status"
        );
    }
    Ok(())
}
