// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use serde_json::{Value, json};

use super::super::{Receipt, TestResult, artifact, index, verify};
use super::{FRAME, catalog, cell, compact_fixture, frame, path, receipt, write_receipt};

#[test]
fn decoding_joins_variant_pools_and_leaves_attempts_outside_the_origin() -> TestResult {
    let raw = compact_fixture(&variant_catalog())?;
    let compressed = frame(&raw)?;
    let directory = tempfile::tempdir()?;
    fs::write(directory.path().join(FRAME), &compressed)?;
    write_receipt(directory.path(), &raw, &compressed)?;

    let binding = Receipt::read(directory.path())?.binding()?;
    let indexed = artifact::read_completed(directory.path(), |input| index(input))?;
    let replay =
        artifact::read_completed(directory.path(), |input| verify(input, &indexed, &binding))?;
    assert_eq!(
        indexed.pass.inventory.variant_set_count(),
        1,
        "the pool is indexed once even when multiple decisions use it"
    );
    assert_eq!(
        replay.sourced_cells, 11,
        "the Item decision, both target cells and both Ship decisions have separate origins"
    );
    assert_eq!(
        indexed.digest,
        index(raw.as_slice())?.digest,
        "the compressed passes preserve the authored references byte for byte"
    );
    assert!(
        !directory.path().join("p4kx-catalog.json").exists(),
        "variant replay leaves no raw artifact"
    );
    Ok(())
}

#[test]
fn invalid_variant_pools_fail_at_the_inventory() -> TestResult {
    for (pointer, invalid, expected) in [
        (
            "/provenance/variantSourceUseSets",
            json!([set(&["u-key"]), set(&["u-lookup"])]),
            "duplicate variant source-use set ID",
        ),
        (
            "/provenance/variantSourceUseSets/0/sourceUsesRefs",
            json!(["u-missing"]),
            "missing referenced source use",
        ),
        (
            "/provenance/variantSourceUseSets/0/sourceUsesRefs",
            json!(["u-attempt"]),
            "noncontributing",
        ),
        (
            "/provenance/variantSourceUseSets/0/sourceUsesRefs",
            json!(["u-key", "u-key"]),
            "sorted and unique",
        ),
    ] {
        let mut document = variant_catalog();
        *path(&mut document, pointer)? = invalid;
        let raw = compact_fixture(&document)?;
        let Err(error) = index(raw.as_slice()) else {
            return Err(format!("expected a variant inventory failure at {pointer}").into());
        };
        assert!(error.to_string().contains(expected), "{pointer}: {error}");
    }
    Ok(())
}

#[test]
fn invalid_variant_origins_fail_when_replaying_decisions() -> TestResult {
    for (pointer, invalid, expected) in [
        (
            "/items/0/variantOf/sourceUseSetRef",
            json!(49),
            "missing variant source-use set",
        ),
        (
            "/items/0/variantOf/sourceUsesRefs",
            json!(["u-key", "u-lookup"]),
            "local and pooled source-use references overlap",
        ),
        (
            "/items/0/variantOf/containsEncryptedSource",
            json!(false),
            "marker disagrees",
        ),
        (
            "/provenance/variantSourceUseSets/0/sourceUsesRefs",
            json!(["u-key", "u-opaque"]),
            "opaque coordinate",
        ),
        (
            "/ships/1/variantOf/failedSourceUseRefs",
            json!(["u-key"]),
            "attempted role",
        ),
    ] {
        let mut document = variant_catalog();
        *path(&mut document, pointer)? = invalid;
        let raw = compact_fixture(&document)?;
        let receipt: Receipt = serde_json::from_value(receipt(&raw, &raw))?;
        let indexed = index(raw.as_slice())?;
        let error = verify(raw.as_slice(), &indexed, &receipt.binding()?)
            .expect_err("invalid decision origin");
        assert!(error.to_string().contains(expected), "{pointer}: {error}");
    }
    Ok(())
}

fn variant_catalog() -> Value {
    let mut document = catalog();
    document["provenance"]["variantSourceUseSets"] = json!([set(&["u-key"])]);
    document["items"][0]["variantOf"] = json!({
        "state": "present",
        "value": {
            "targetRecordIndex": cell(&json!(4), Some(false), true, &["u-lookup"]),
            "guid": cell(&json!("fixture"), Some(false), true, &["u-lookup"]),
        },
        "containsEncryptedSource": true,
        "sourceSetComplete": true,
        "sourceUsesRefs": ["u-lookup"],
        "sourceUseSetRef": 17,
        "failedSourceUseRefs": []
    });
    document["ships"] = json!([
        {"variantOf": {
            "state": "absent", "value": null, "containsEncryptedSource": true,
            "sourceSetComplete": true, "sourceUsesRefs": [], "sourceUseSetRef": 17,
            "failedSourceUseRefs": []
        }},
        {"variantOf": {
            "state": "unreadable", "value": null, "containsEncryptedSource": null,
            "sourceSetComplete": false, "sourceUsesRefs": [], "sourceUseSetRef": null,
            "failedSourceUseRefs": ["u-attempt"]
        }}
    ]);
    document
}

fn set(references: &[&str]) -> Value {
    json!({"id": 17, "sourceUsesRefs": references})
}
