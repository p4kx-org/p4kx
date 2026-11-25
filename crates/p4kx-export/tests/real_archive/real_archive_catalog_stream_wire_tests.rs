// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::de::DeserializeSeed;
use serde_json::{Value, json};

use super::super::digest_runs::{DigestRuns, OriginDigest};
use super::{OrderedRows, StreamOrigins, TestResult, fingerprint, wire_slot};

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
            id: id.into(),
            payload: fingerprint(row.clone())?,
        })?;
    }
    let run = runs.finish()?;
    let seed = StreamOrigins(&run);
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

#[test]
fn streamed_source_comparison_handles_sparse_origins_after_external_merges() -> TestResult {
    use std::io::Write as _;

    const ORIGINS: usize = 250_000;
    const SPACING: usize = 20;
    let scratch = tempfile::tempdir()?;
    let mut runs = DigestRuns::new(scratch.path());
    let mut input = Vec::new();
    input.write_all(b"[")?;
    for number in 0..ORIGINS {
        let id = format!("u10:a0.32:payload:payload:metadata:{number:06}");
        let wire = json!({"id": &id, "field": "numeric", "value": number});
        if number != 0 {
            input.write_all(b",")?;
        }
        serde_json::to_writer(&mut input, &wire)?;
        if number.is_multiple_of(SPACING) {
            runs.push(OriginDigest {
                id: id.into_boxed_str(),
                payload: fingerprint(wire)?,
            })?;
        }
    }
    input.write_all(b"]")?;
    let run = runs.finish()?;
    let mut decoder = serde_json::Deserializer::from_reader(input.as_slice());
    assert_eq!(
        StreamOrigins(&run).deserialize(&mut decoder)?,
        json!(ORIGINS / SPACING),
        "sparse selected IDs survive a complete buffered wire comparison"
    );
    decoder.end()?;
    Ok(())
}

#[test]
fn authored_encounter_comparison_separates_count_order_and_wire_content() -> TestResult {
    let first = json!({"instanceId":{"state":"present","value":{
        "rootEntryIndex": 91,
        "childNodeIndices": [3, 5],
        "externalChildStepIndices": [1]
    }},"name":"external"});
    let second = json!({"instanceId":{"state":"present","value":{
        "rootEntryIndex": 7,
        "childNodeIndices": [3, 5],
        "externalChildStepIndices": []
    }},"name":"inline"});
    let mut ordered = OrderedRows::in_encounter_order();
    for row in [&first, &second] {
        ordered.append_encounter_row(super::encounter_key(row)?, fingerprint(row.clone())?)?;
    }
    let expected = ordered.finish();
    assert_eq!(
        compare_json(expected, &json!([first, second]))?,
        json!(2),
        "authored order is independent of numeric root Entry order"
    );
    let mut different = second.clone();
    *different.get_mut("name").ok_or("authored name fixture")? = json!("changed");
    for (actual, diagnostic) in [
        (json!([first]), "row count differs"),
        (
            json!([second, first]),
            "identities or encounter order differ",
        ),
        (
            json!([first, different]),
            "wire digests differ with matching identities and counts",
        ),
    ] {
        let error = compare_json(expected, &actual)
            .err()
            .ok_or("comparison accepted a changed row array")?;
        assert!(
            error.to_string().contains(diagnostic),
            "discriminating error: {error}"
        );
    }
    Ok(())
}
