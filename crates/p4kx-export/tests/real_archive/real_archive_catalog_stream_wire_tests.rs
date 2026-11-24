// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::de::DeserializeSeed;
use serde_json::{Value, json};

use super::super::digest_runs::DigestRuns;
use super::{
    DigestIndex, OrderedRows, OriginDigest, StreamOrigins, TestResult, fingerprint, wire_slot,
};

fn compare_json(
    seed: impl for<'de> DeserializeSeed<'de, Value = Value>,
    value: &Value,
) -> TestResult<Value> {
    let bytes = serde_json::to_vec(value)?;
    let mut decoder = serde_json::Deserializer::from_slice(&bytes);
    let output = seed.deserialize(&mut decoder)?;
    decoder.end()?;
    Ok(output)
}

#[test]
fn streamed_rows_refuse_different_count_order_ownership_and_payloads() -> TestResult {
    let first = json!({
        "ownerRecordIndex": {"state": "present", "value": 9},
        "componentOrdinal": {"state": "present", "value": 2},
        "original": {"state": "present", "value": "first"}
    });
    let second = json!({
        "ownerRecordIndex": {"state": "present", "value": 9},
        "componentOrdinal": {"state": "present", "value": 7},
        "original": {"state": "null", "value": null}
    });
    let mut expected = OrderedRows::default();
    for row in [&first, &second] {
        expected.append(wire_slot(row)?, fingerprint(row.clone())?)?;
    }
    let expected = expected.finish();
    assert_eq!(
        compare_json(expected, &json!([first, second]))?,
        json!(2),
        "both physical slots match"
    );
    for changed in [
        json!([second, first]),
        json!([first, first]),
        json!([second]),
        json!([first, second, second]),
        json!([first, {"ownerRecordIndex":{"state":"absent","value":9},"componentOrdinal":{"state":"present","value":7}}]),
        json!([first, {"ownerRecordIndex":{"state":"present","value":9},"componentOrdinal":{"state":"present","value":7},"original":{"state":"absent","value":null}}]),
    ] {
        assert!(
            compare_json(expected, &changed).is_err(),
            "the physical sequence is exact: {changed}"
        );
    }
    Ok(())
}

#[test]
fn streamed_source_membership_refuses_conflicts_missing_and_unordered_ids() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let first_id = "a0:d0:9:Components[2]:payload";
    let second_id = "z:1:ancestor";
    let first = json!({"id":first_id,"source":"owned", "location":{"offset":11}});
    let unrelated = json!({"id":"an-unselected-id","source":"independent"});
    let second = json!({"id":second_id,"source":"ancestor", "location":{"offset":28}});
    let mut runs = DigestRuns::new(scratch.path());
    for (id, row) in [(second_id, &second), (first_id, &first)] {
        runs.push(OriginDigest {
            id: OriginDigest::id_hash(id),
            payload: fingerprint(row.clone())?,
        })?;
    }
    let index = DigestIndex::open(runs.finish()?)?;
    let seed = StreamOrigins(&index);
    assert_eq!(
        compare_json(seed, &json!([first, unrelated, second]))?,
        json!(2),
        "only the exact referenced subset is compared"
    );
    for changed in [
        json!([first, unrelated]),
        json!([first, unrelated, unrelated, second]),
        json!([unrelated, first, second]),
        json!([first, first, second]),
        json!([first, {"id":second_id,"source":"ancestor","location":{"offset":29}}]),
        json!([first, {"source":"missing-id"}, second]),
    ] {
        assert!(
            compare_json(seed, &changed).is_err(),
            "invalid source inventory is refused: {changed}"
        );
    }
    Ok(())
}
