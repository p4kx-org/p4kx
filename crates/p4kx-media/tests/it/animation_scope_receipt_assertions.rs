// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::Archive;
use p4kx_media::MediaRecord;
use serde_json::Value;

use super::{BLOCKED_CDF, CDF, TestResult};

const SCOPE_RECEIPT: &str = "animation-definition-scope-receipt";

pub(super) fn verify_root_exclusion(archive: &Archive, rows: &[MediaRecord]) -> TestResult {
    let [receipt] = rows else {
        return Err("expected one root exclusion receipt".into());
    };
    assert_eq!(
        receipt.subresource.namespace, SCOPE_RECEIPT,
        "root exclusion is a scope result"
    );
    assert_eq!(
        scope_field(receipt, "outcome/state"),
        Some("excluded"),
        "the model selector is explicitly excluded"
    );
    assert_eq!(
        scope_field(receipt, "outcome/entry_index"),
        Some(source_index(archive, CDF)?.as_str()),
        "the exclusion names the physical root"
    );
    Ok(())
}

pub(super) fn verify_scope_receipts(archive: &Archive, rows: &[MediaRecord]) -> TestResult {
    let expected_root = source_index(archive, "Data/Game2.dcb")?;
    let expected_parameter = source_index(archive, "Data/Objects/shared.chrparams")?;
    let expected_excluded = source_index(archive, BLOCKED_CDF)?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let mut indices = BTreeSet::new();
    let mut selected_owners = Vec::new();
    let mut outcomes = Vec::new();
    for row in rows
        .iter()
        .filter(|row| row.subresource.namespace == SCOPE_RECEIPT)
    {
        schema
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
        assert_eq!(
            scope_field(row, "input_entry_index"),
            Some(expected_root.as_str()),
            "every occurrence belongs to the original physical DataForge",
        );
        assert!(
            indices.insert(&row.subresource.indices),
            "one receipt per resolved occurrence"
        );
        let state = scope_field(row, "outcome/state").ok_or("coverage disposition")?;
        if let Some(owner) = receipt_owner(row, state, &expected_parameter, &expected_excluded)? {
            selected_owners.push(owner);
        }
        outcomes.push(state);
    }
    outcomes.sort_unstable();
    assert_eq!(
        outcomes,
        [
            "excluded",
            "no-companion",
            "no-rig",
            "selected",
            "selected",
            "selected"
        ],
        "all four scope outcomes preserve their original multiplicity"
    );
    let [root, first, second] = selected_owners.as_slice() else {
        return Err("expected selected occurrence owners".into());
    };
    assert_ne!(
        root, first,
        "the root and child have separate Record identities"
    );
    assert_eq!(
        first, second,
        "two placements can share the same owner and parameter Entry"
    );
    Ok(())
}

fn receipt_owner(
    row: &MediaRecord,
    state: &str,
    expected_parameter: &str,
    expected_excluded: &str,
) -> Result<Option<u64>, Box<dyn std::error::Error>> {
    let entry = scope_field(row, "outcome/entry_index");
    match state {
        "selected" => {
            assert_eq!(
                entry,
                Some(expected_parameter),
                "the admitted parameter is physical"
            );
            let owner = row
                .data
                .pointer("/payload/owner_record_index")
                .and_then(Value::as_u64)
                .ok_or("physical owner Record")?;
            Ok(Some(owner))
        }
        "excluded" => {
            assert_eq!(
                entry,
                Some(expected_excluded),
                "the excluded CDF stays addressable"
            );
            Ok(None)
        }
        _ => {
            assert_eq!(entry, None, "absence does not invent an Entry");
            Ok(None)
        }
    }
}

fn scope_field<'record>(row: &'record MediaRecord, field: &str) -> Option<&'record str> {
    row.data
        .pointer(&format!("/payload/{field}"))
        .and_then(Value::as_str)
}

fn source_index(archive: &Archive, path: &str) -> Result<String, Box<dyn std::error::Error>> {
    archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .map(|entry| entry.index().to_string())
        .ok_or_else(|| "physical fixture Entry".into())
}
