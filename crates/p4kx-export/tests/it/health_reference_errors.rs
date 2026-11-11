// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Field, Value as Raw, ValueError};
use serde_json::{Value, json};

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{self as fixture, Input, TestResult};
use crate::health_origin_reference_source as source;
use crate::health_reference_assertions as references;
use crate::health_reference_fixture;
use crate::health_reference_origins as origins;

#[test]
fn health_reference_original_error_tokens_keep_guid_words_and_inapplicable_target_cells()
-> TestResult {
    let bytes = source::database();
    let words = source::original(&bytes)?;
    let forge = DataForge::open(&bytes)?;
    let owner = forge.record(1).ok_or("actual error owner")?;
    let tree = owner.tree()?;
    let fields: Vec<_> = tree.root().fields().iter().map(Field::name).collect();
    assert_eq!(
        fields,
        ["Code", "Resolved", "Dangling", "Owned", "Again", "Weak"],
        "the original field/container order fixes each independent source ordinal"
    );
    assert!(
        matches!(
            tree.field("Again"),
            Some(Raw::Error(ValueError::OwnedTwice {
                struct_index: 1,
                instance_index: 0
            }))
        ),
        "the genuine repeated strong pointer retains both original words"
    );
    for input in [Input::Plain, Input::Encrypted] {
        let catalog = health_reference_fixture::document(&bytes, input)?;
        dangling(&catalog, input.marker())?;
        pointer(&catalog, "invalidPointer", words, 6, input.marker())?;
        pointer(&catalog, "ownedTwice", (1, 0), 5, input.marker())?;
        check::closure(&catalog)?;
    }
    Ok(())
}

fn by_status<'document>(catalog: &'document Value, status: &str) -> TestResult<&'document Value> {
    references::rows(catalog)?
        .iter()
        .find(|row| row.get("status") == Some(&json!(status)))
        .ok_or_else(|| format!("actual original unresolved {status}").into())
}

fn dangling(catalog: &Value, marker: Option<bool>) -> TestResult {
    let row = by_status(catalog, "dangling")?;
    let guid = check::present(
        row,
        "targetGuid",
        &json!(fixture::guid(source::MISSING)),
        marker,
    )?;
    origins::exact(catalog, guid, &[origins::body(2, "$error.guid", "payload")])?;
    check::present_text(catalog, row, "field", &json!("Dangling"), marker)?;
    for field in [
        "targetRecordIndex",
        "targetRecordPath",
        "targetType",
        "targetLocation",
    ] {
        references::absent(row, field)?;
    }
    assert_eq!(
        row.get("sourcePointer"),
        Some(&Value::Null),
        "an authored dangling GUID has no fabricated pointer group"
    );
    Ok(())
}

pub(crate) fn pointer(
    catalog: &Value,
    status: &str,
    words: (u32, u32),
    ordinal: u64,
    marker: Option<bool>,
) -> TestResult {
    let row = by_status(catalog, status)?;
    let pointer = check::at(row, "/sourcePointer")?;
    for (field, token, value) in [
        ("structIndex", "$error.structIndex", words.0),
        ("instanceIndex", "$error.instanceIndex", words.1),
    ] {
        let cell = check::present(pointer, field, &json!(value), marker)?;
        origins::exact(catalog, cell, &[origins::body(ordinal, token, "payload")])?;
    }
    references::absent(pointer, "declaredTypeIndex")?;
    for field in [
        "targetGuid",
        "targetRecordIndex",
        "targetRecordPath",
        "targetType",
        "targetLocation",
    ] {
        references::absent(row, field)?;
    }
    Ok(())
}
