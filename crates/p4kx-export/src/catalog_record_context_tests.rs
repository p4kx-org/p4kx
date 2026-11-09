// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{
    InputScope, SourceInput, SourceLocation, SourceUse, SourceUseRole, SourceUses, SourceValue,
};
use serde_json::{Value, json};

use super::CatalogRecordContext;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn literal_body_header_labels_and_dollar_names_keep_distinct_original_sites() -> TestResult {
    let uses = SourceUses::new();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    let context = CatalogRecordContext::new(&uses, &scope, 7);
    let header = context.header_value(
        RecordHeaderField::RecordName,
        SourceValue::Present("header"),
    )?;
    for field in ["$name", "recordName"] {
        let body = context.value(field, SourceValue::Present("body"))?;
        assert_ne!(
            body.origin().source_uses_refs(),
            header.origin().source_uses_refs(),
            "the explicit header API never routes a body label by prefix or spelling"
        );
        let observed = context.observe(field, SourceUseRole::Payload)?;
        assert!(
            matches!(observed.description().location(), SourceLocation::DataForge { record_index: 7, field: original } if original == field),
            "ordinary value/observe operations retain the exact original body path"
        );
    }
    assert_eq!(
        header.value(),
        Some(&"header"),
        "coordinate separation does not change retained values"
    );
    assert!(
        header.origin().contains_encrypted_source().is_none(),
        "an unknown original source is never promoted to plain"
    );
    Ok(())
}

#[test]
fn attempted_header_metadata_retains_its_observation_without_a_false_contribution() -> TestResult {
    let uses = SourceUses::new();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    let context = CatalogRecordContext::new(&uses, &scope, 5);
    let value =
        context.header_value::<&str>(RecordHeaderField::MetadataTag, SourceValue::Unreadable)?;
    assert!(
        !value.origin().source_set_complete(),
        "a failed original read cannot claim complete contributors"
    );
    assert!(
        value.origin().source_uses_refs().is_empty(),
        "attempts do not become retained input values"
    );
    let inventory = serde_json::to_value(uses.freeze()?)?;
    let rows = inventory
        .get("uses")
        .and_then(Value::as_array)
        .ok_or("actual observation inventory")?;
    assert!(rows.iter().any(|row| row.get("role") == Some(&json!("attempted")) && row.get("location") == Some(&json!({"type":"subresource", "namespace":"dataforgeRecordHeader", "indices":["5"], "field":"metadataTag"}))), "the failed original header coordinate remains observable separately");
    Ok(())
}

#[test]
fn normalized_path_selection_uses_filename_conversion_and_actual_branch_role() -> TestResult {
    let uses = SourceUses::new();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    let context = CatalogRecordContext::new(&uses, &scope, 2);
    for (role, expected) in [
        (
            SourceUseRole::Selector,
            vec![SourceUseRole::Conversion, SourceUseRole::Selector],
        ),
        (SourceUseRole::Attempted, vec![SourceUseRole::Attempted]),
        (SourceUseRole::Conversion, vec![SourceUseRole::Conversion]),
    ] {
        let mut observed = Vec::new();
        context.append_header_input(RecordHeaderInput::NormalizedPath, role, &mut observed)?;
        assert_eq!(
            observed.iter().map(SourceUse::role).collect::<Vec<_>>(),
            expected,
            "a failed read does not invent a successful filename conversion"
        );
        for used in observed {
            assert_eq!(
                serde_json::to_value(used.description().location())?,
                json!({"type":"subresource", "namespace":"dataforgeRecordHeader", "indices":["2"], "field":"fileName"}),
                "the original filename supplies the normalization input"
            );
        }
    }
    Ok(())
}
