// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::de::DeserializeSeed as _;
use serde_json::json;

use crate::{
    FieldOrigin, FieldOriginSeed, FieldOrigins, FieldOriginsSeed, FieldPath, FieldPathSeed,
    InputScope, OriginReaderLimits, SourceInput, SourceLocation, SourceUseRole, SourceUses,
    SourceValue,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(crate) const fn reader_limits() -> OriginReaderLimits {
    OriginReaderLimits {
        fields: 1024,
        metadata_bytes: 2 << 20,
        path_bytes: 16 << 10,
        field_uses: 4096,
        reference_bytes: 1 << 20,
    }
}

fn read_origin(
    value: serde_json::Value,
    limits: OriginReaderLimits,
) -> Result<FieldOrigin, serde_json::Error> {
    FieldOriginSeed::new(limits).deserialize(value)
}

#[test]
fn origin_reader_limits_do_not_cap_shared_writer_aggregates() -> TestResult {
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let handles = (0..4097)
        .map(|ordinal| {
            scope.at(
                SourceLocation::DataForge {
                    record_index: ordinal / 4096,
                    field: format!("value[{}]", ordinal % 4096),
                },
                SourceUseRole::Payload,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let value = sources.value(SourceValue::Present(()), &handles, true);
    assert_eq!(
        value.origin().source_uses_refs().len(),
        4097,
        "the producer retains an aggregate spanning multiple records"
    );
    let encoded = serde_json::to_value(value.origin())?;
    assert!(
        read_origin(encoded.clone(), reader_limits()).is_err(),
        "the consumer's per-field policy rejects without truncation"
    );
    let limits = OriginReaderLimits {
        field_uses: 8192,
        ..reader_limits()
    };
    let decoded = read_origin(encoded, limits)?;
    assert_eq!(
        decoded.source_uses_refs(),
        value.origin().source_uses_refs(),
        "an explicitly larger reader preserves every original use"
    );
    assert_eq!(
        decoded.contains_encrypted_source(),
        None,
        "a bounded read never invents unknown input lineage"
    );
    assert!(
        decoded.source_set_complete(),
        "known contributor enumeration and physical encryption certainty stay separate"
    );
    Ok(())
}

#[test]
fn reader_path_and_reference_bounds_apply_only_to_each_explicit_policy() -> TestResult {
    let path_text = format!("/{}", "x".repeat((16 << 10) + 1));
    let path = FieldPath::parse(&path_text)?;
    let wire = serde_json::to_string(&path)?;
    assert!(
        FieldPathSeed::new(16 << 10)
            .deserialize(&mut serde_json::Deserializer::from_str(&wire))
            .is_err(),
        "a restrictive reader rejects a longer producer path"
    );
    let decoded = FieldPathSeed::new(path_text.len())
        .deserialize(&mut serde_json::Deserializer::from_str(&wire))?;
    assert_eq!(
        decoded, path,
        "the explicit exact path-byte boundary round-trips"
    );
    let value = json!({"containsEncryptedSource": null, "sourceUsesRefs": ["four"], "sourceSetComplete": true});
    assert!(
        read_origin(
            value.clone(),
            OriginReaderLimits {
                reference_bytes: 3,
                ..reader_limits()
            }
        )
        .is_err(),
        "references are checked before their owned copy"
    );
    let decoded = read_origin(
        value,
        OriginReaderLimits {
            reference_bytes: 4,
            ..reader_limits()
        },
    )?;
    assert_eq!(
        decoded.source_uses_refs()[0].as_str(),
        "four",
        "the exact reference-byte boundary is retained"
    );
    Ok(())
}

#[test]
fn total_metadata_budget_is_shared_across_field_origins() -> TestResult {
    let sources = SourceUses::new();
    let origin = sources
        .value(SourceValue::Present(()), &[], true)
        .origin()
        .clone();
    let mut fields = FieldOrigins::new();
    fields.insert(FieldPath::parse("/a")?, origin.clone())?;
    fields.insert(FieldPath::parse("/b")?, origin)?;
    let encoded = serde_json::to_string(&fields)?;
    let one_field_bytes = 2 + size_of::<FieldPath>() + size_of::<FieldOrigin>();
    let limits = OriginReaderLimits {
        metadata_bytes: one_field_bytes * 2,
        ..reader_limits()
    };
    let decoded = FieldOriginsSeed::new(limits)
        .deserialize(&mut serde_json::Deserializer::from_str(&encoded))?;
    assert_eq!(
        decoded.iter().count(),
        2,
        "the exact total logical storage boundary retains both fields"
    );
    assert!(
        FieldOriginsSeed::new(OriginReaderLimits {
            metadata_bytes: one_field_bytes * 2 - 1,
            ..limits
        })
        .deserialize(&mut serde_json::Deserializer::from_str(&encoded))
        .is_err(),
        "later fields cannot reset the shared byte budget"
    );
    assert!(
        FieldOriginsSeed::new(OriginReaderLimits {
            fields: 1,
            ..reader_limits()
        })
        .deserialize(&mut serde_json::Deserializer::from_str(&encoded))
        .is_err(),
        "field cardinality has a separate explicit reader bound"
    );
    Ok(())
}

#[test]
fn duplicate_origin_keys_and_reference_count_overflow_are_rejected() {
    let duplicate = r#"{"containsEncryptedSource":null,"containsEncryptedSource":false,"sourceUsesRefs":[],"sourceSetComplete":false}"#;
    assert!(
        FieldOriginSeed::new(reader_limits())
            .deserialize(&mut serde_json::Deserializer::from_str(duplicate))
            .is_err(),
        "a nullable first occurrence still counts as a present key"
    );
    let value = json!({"containsEncryptedSource": null, "sourceUsesRefs": ["one", "two"], "sourceSetComplete": true});
    assert!(
        read_origin(
            value,
            OriginReaderLimits {
                field_uses: 1,
                ..reader_limits()
            }
        )
        .is_err(),
        "an additional reference cannot bypass the count bound through a missing sequence size hint"
    );
}
