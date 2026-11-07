// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::shader_support::{self as support, TestResult};

fn fixture() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(
        folder.path(),
        [
            p4kx_testkit::TestEntry::stored(
                "source.ext",
                b"Version(1) Property { Name(%A); Mask(0xffffffffffffffff); Mask(17); Hidden; }",
            ),
            p4kx_testkit::TestEntry::stored("source.shcb", b"opaque synthetic cache"),
        ],
    )?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &support::options(),
        &support::registry(p4kx_media::ShaderProvider::default())?,
    )?)?;
    support::records(&output, &index)?
        .into_iter()
        .map(|record| serde_json::to_value(record).map_err(Into::into))
        .collect()
}

fn namespace<'record>(
    records: &'record [Value],
    name: &str,
) -> Result<&'record Value, Box<dyn std::error::Error>> {
    records
        .iter()
        .find(|record| {
            record
                .pointer("/subresource/namespace")
                .and_then(Value::as_str)
                == Some(name)
        })
        .ok_or_else(|| format!("missing fixture namespace {name}").into())
}

#[test]
fn actual_shader_payloads_and_namespace_bound_records_validate() -> TestResult {
    let record_schema = p4kx_media::shader_record_schema()?;
    let payload_schema = p4kx_media::shader_payload_schema();
    let record_validator = jsonschema::validator_for(&record_schema)?;
    let payload_validator = jsonschema::validator_for(&payload_schema)?;
    for record in fixture()? {
        record_validator
            .validate(&record)
            .map_err(|error| error.to_string())?;
        payload_validator
            .validate(record.get("data").ok_or("actual payload")?)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[test]
fn integer_word_schema_rejects_out_of_width_noncanonical_and_numeric_mutations() -> TestResult {
    let records = fixture()?;
    let original = records
        .iter()
        .find(|record| {
            record
                .pointer("/data/payload/meaning/interpretation/state")
                .and_then(Value::as_str)
                == Some("parsed")
        })
        .ok_or("parsed mask fixture")?;
    let validator = jsonschema::validator_for(&p4kx_media::shader_record_schema()?)?;
    for word in [
        serde_json::json!("18446744073709551616"),
        serde_json::json!("-1"),
        serde_json::json!("01"),
        serde_json::json!("+1"),
        serde_json::json!("1\n"),
        serde_json::json!(18_446_744_073_709_551_615_u64),
        serde_json::json!(false),
    ] {
        let mut mutation = original.clone();
        support::replace(
            &mut mutation,
            "/data/payload/meaning/interpretation/word",
            word,
        )?;
        assert!(
            !validator.is_valid(&mutation),
            "lossless word schema rejects value, representation and width changes"
        );
    }
    Ok(())
}

#[test]
fn namespace_payload_mismatch_missing_nullable_field_and_foreign_spans_are_refused() -> TestResult {
    let records = fixture()?;
    let validator = jsonschema::validator_for(&p4kx_media::shader_record_schema()?)?;
    let header = namespace(&records, "shader-definition")?;
    let mut mismatch = header.clone();
    support::replace(
        &mut mismatch,
        "/subresource/namespace",
        serde_json::json!("shader-declaration"),
    )?;
    assert!(
        !validator.is_valid(&mismatch),
        "kind alone cannot hide namespace/payload disagreement"
    );
    let mut missing = namespace(&records, "shader-declaration")?.clone();
    let payload = missing
        .pointer_mut("/data/payload")
        .and_then(Value::as_object_mut)
        .ok_or("declaration object")?;
    assert!(
        payload.remove("parent").is_some(),
        "nullable fixture field must exist before mutation"
    );
    assert!(
        !validator.is_valid(&missing),
        "explicit nullable fields remain required in the payload schema"
    );
    for (pointer, value) in [
        ("/data/payload/span/basis", serde_json::json!("section")),
        ("/data/payload/span/start", serde_json::json!(-1)),
        ("/data/payload/counts/definitions", serde_json::json!(0)),
    ] {
        let mut mutation = header.clone();
        support::replace(&mut mutation, pointer, value)?;
        assert!(
            !validator.is_valid(&mutation),
            "source basis, unsigned offsets and counting units are schema constraints"
        );
    }
    Ok(())
}

#[test]
fn cache_schema_does_not_accept_decoded_claims_or_definition_counts() -> TestResult {
    let records = fixture()?;
    let cache = namespace(&records, "shader-cache-inventory")?;
    let validator = jsonschema::validator_for(&p4kx_media::shader_record_schema()?)?;
    for (pointer, value) in [
        (
            "/data/payload/semantic_decode",
            serde_json::json!("decoded"),
        ),
        ("/data/payload/definitions", serde_json::json!(1)),
        ("/data/payload/properties", serde_json::json!(1)),
        ("/data/payload/compiled_caches", serde_json::json!(0)),
        ("/status", serde_json::json!("partial")),
    ] {
        let mut mutation = cache.clone();
        support::replace(&mut mutation, pointer, value)?;
        assert!(
            !validator.is_valid(&mutation),
            "opaque cache inventory cannot claim decoded shader vocabulary or inconsistent status"
        );
    }
    Ok(())
}

#[test]
fn custom_shader_namespaces_keep_the_generic_record_contract() -> TestResult {
    let records = fixture()?;
    let mut custom = namespace(&records, "shader-definition")?.clone();
    support::replace(
        &mut custom,
        "/subresource/namespace",
        serde_json::json!("custom.shader"),
    )?;
    support::replace(
        &mut custom,
        "/data",
        serde_json::json!({"custom":"independent schema"}),
    )?;
    let validator = jsonschema::validator_for(&p4kx_media::shader_record_schema()?)?;
    validator
        .validate(&custom)
        .map_err(|error| error.to_string())?;
    support::replace(
        &mut custom,
        "/subresource/namespace",
        serde_json::json!("shader-definition"),
    )?;
    assert!(
        !validator.is_valid(&custom),
        "only claiming the built-in namespace activates its specific schema"
    );
    support::replace(&mut custom, "/kind", serde_json::json!("model"))?;
    validator
        .validate(&custom)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn unsupported_mask_inventory_status_is_schema_distinct_from_malformed_literal_status() -> TestResult
{
    let records = fixture()?;
    let original = records
        .iter()
        .find(|record| {
            record
                .pointer("/data/payload/meaning/interpretation/state")
                .and_then(Value::as_str)
                == Some("unknown-radix")
        })
        .ok_or("unprefixed mask fixture")?;
    let validator = jsonschema::validator_for(&p4kx_media::shader_record_schema()?)?;
    let mut mutation = original.clone();
    support::replace(&mut mutation, "/status", serde_json::json!("partial"))?;
    assert!(
        !validator.is_valid(&mutation),
        "unclaimed general interpretation is complete raw inventory"
    );
    support::replace(&mut mutation, "/status", serde_json::json!("complete"))?;
    support::replace(
        &mut mutation,
        "/data/payload/meaning/interpretation/state",
        serde_json::json!("malformed"),
    )?;
    assert!(
        !validator.is_valid(&mutation),
        "known-invalid supported hexadecimal literal cannot claim complete interpretation"
    );
    support::replace(&mut mutation, "/status", serde_json::json!("partial"))?;
    assert!(
        !validator.is_valid(&mutation),
        "partial literal failure needs a projection-loss issue, not an inventory-only info"
    );
    support::replace(
        &mut mutation,
        "/issues/0/severity",
        serde_json::json!("warning"),
    )?;
    validator
        .validate(&mutation)
        .map_err(|error| error.to_string())?;
    Ok(())
}
