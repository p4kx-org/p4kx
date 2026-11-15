// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::{A, B, C, H, SUB_ONE, TestResult, build, entry, v3};
use crate::commodity_v3_fixture::text_row;
use crate::health_origin_assertions as proof;

#[test]
fn each_contractor_occurrence_uses_the_nearest_eligible_layer() -> TestResult {
    let (_, catalog) = build(None)?;
    for (guid, sub, key, display) in [
        (A, None, "@contractor_acme", "Acme Agency"),
        (A, Some(SUB_ONE), "@contractor_acme", "Acme Agency"),
        (B, None, "@contractor_acme", "Acme Agency"),
        (H, None, "@contractor_local", "Local Agency"),
    ] {
        let row = entry(&catalog, guid, sub)?;
        let carrier = proof::cell(row, "contractor", "present")?;
        let chosen = text_row(&catalog, "gameTexts", &carrier["value"])?;
        assert_eq!(
            chosen.pointer("/key/value").and_then(Value::as_str),
            Some(key),
            "the selected layer supplies its original key"
        );
        assert_eq!(
            chosen
                .pointer("/translations/value/en/displayText/value")
                .and_then(Value::as_str),
            Some(display),
            "the canonical text resolves the expected label"
        );
        check_contractor_payload(&catalog, row, carrier, chosen)?;
    }
    let mut keyed = 0;
    for row in v3::rows(&catalog)? {
        keyed += usize::from(v3::cell(row, "contractor")?.is_some());
    }
    assert_eq!(
        keyed, 7,
        "each inherited keyed occurrence enters the text baseline"
    );
    let empty = proof::cell(entry(&catalog, C, None)?, "contractor", "absent")?;
    assert!(
        !proof::uses(&catalog, empty)?.is_empty(),
        "the surveyed layers remain traceable"
    );
    assert!(
        proof::rows(&catalog, "/texts/gameTexts")?
            .iter()
            .all(|row| row.pointer("/key/value").and_then(Value::as_str)
                != Some("@contractor_bypassed")),
        "a later Contractor entry does not replace the first matching entry in its layer"
    );
    Ok(())
}

fn check_contractor_payload(
    catalog: &Value,
    contract: &Value,
    carrier: &Value,
    chosen: &Value,
) -> TestResult {
    let uses = proof::uses(catalog, carrier)?;
    let payloads = uses
        .iter()
        .filter(|used| used["role"] == "payload")
        .copied()
        .collect::<Vec<_>>();
    let [payload] = payloads.as_slice() else {
        return Err("contractor needs one authored key payload".into());
    };
    assert_eq!(
        payload
            .pointer("/location/namespace")
            .and_then(Value::as_str),
        Some("dataforgeValue"),
        "the selected label is sourced to its original value slot"
    );
    let record = contract
        .pointer("/source/recordIndex")
        .and_then(Value::as_u64)
        .ok_or("Generator record")?;
    assert_eq!(
        payload.pointer("/location/indices/0"),
        Some(&json!(record.to_string())),
        "the payload belongs to the original Generator Record"
    );
    let lookups = proof::uses(catalog, &chosen["key"])?;
    assert!(
        lookups.contains(payload),
        "the registry uses the same physical authored key"
    );
    Ok(())
}
