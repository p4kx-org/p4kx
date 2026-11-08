// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Canonical resource wire checks for the headless GUI catalog adapter.

use serde_json::{Value, json};

use crate::cataloging::TestResult;

/// Checks the canonical resource row and the validation flag's direct source origin.
pub(crate) fn assert_written_resource(document: &Value, count: usize) -> TestResult {
    assert!(
        document.get("resourceTypes").is_none(),
        "no duplicate legacy resource root"
    );
    let rows = document
        .pointer("/definitions/resourceTypes/rows")
        .and_then(Value::as_array)
        .ok_or("no canonical resource rows")?;
    assert_eq!(rows.len(), count, "file and GUI summary agree");
    let flag = rows
        .first()
        .and_then(|row| row.get("validateDefaultCargoBox"))
        .ok_or("no cargo-validation carrier")?;
    assert_eq!(
        flag["state"], "present",
        "false is a present authored value"
    );
    assert_eq!(flag["value"], false, "false remains authored data");
    assert_eq!(
        flag["containsEncryptedSource"], false,
        "the direct marker identifies the unencrypted input"
    );
    assert_eq!(
        flag["sourceSetComplete"], true,
        "the owning cell has complete contributor evidence"
    );
    assert_flag_origin(document, flag)
}

fn assert_flag_origin(document: &Value, flag: &Value) -> TestResult {
    let references = flag
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("no immediate flag source references")?;
    assert_eq!(
        references.len(),
        1,
        "the authored Bool cell contributes once"
    );
    let reference = references.first().ok_or("no flag source reference")?;
    let uses = document
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("no source-use inventory")?;
    let contribution = uses
        .iter()
        .find(|contribution| contribution.get("id") == Some(reference))
        .ok_or("the flag source reference must resolve")?;
    assert_eq!(
        contribution["scope"], "payload",
        "the retained value is payload evidence"
    );
    assert_eq!(
        contribution["role"], "payload",
        "the authored cell contributes its payload"
    );
    assert_eq!(
        contribution["location"],
        json!({
            "type": "dataForge", "recordIndex": 0, "field": "validateDefaultCargoBox"
        }),
        "the flag comes from the exact owning source cell"
    );
    let source_id = contribution
        .get("source")
        .ok_or("no physical source reference")?;
    let sources = document
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("no physical source inventory")?;
    let source = sources
        .iter()
        .find(|source| source.get("id") == Some(source_id))
        .ok_or("the actual flag input must resolve")?;
    assert_eq!(source["entryIndex"], 0, "the fixture's DataForge Entry");
    assert_eq!(
        source["storedEncryption"], "unencrypted",
        "the field marker retains the physical source declaration"
    );
    Ok(())
}
