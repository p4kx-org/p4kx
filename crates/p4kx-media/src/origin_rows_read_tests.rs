// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::OriginReaderLimits;
use serde::de::DeserializeSeed;

use crate::origin_rows_read::{ReadRow, RowSeed};

fn limits() -> OriginReaderLimits {
    OriginReaderLimits {
        fields: 10,
        metadata_bytes: 4096,
        path_bytes: 256,
        field_uses: 8,
        reference_bytes: 256,
    }
}

#[test]
fn field_row_order_is_independent_and_nullable_marker_is_required() {
    let bytes = br#"{"sourceSetComplete":false,"sourceUsesRefs":[],"field":"/data/~0~1","scope":{"ordinal":"0","stream":{"type":"sources"}},"containsEncryptedSource":null,"type":"field"}"#;
    let row = RowSeed(limits())
        .deserialize(&mut serde_json::Deserializer::from_slice(bytes))
        .expect("field row keys may appear in any order");
    let ReadRow::Field { field, origin, .. } = row else {
        panic!("field variant");
    };
    assert_eq!(field, "/data/~0~1", "exact original escaped pointer");
    assert_eq!(
        origin.contains_encrypted_source(),
        None,
        "required explicit null retained"
    );
    assert!(
        !origin.source_set_complete(),
        "incomplete remains independent"
    );
    let missing = br#"{"type":"field","scope":{"stream":{"type":"sources"},"ordinal":"0"},"field":"","sourceUsesRefs":[],"sourceSetComplete":false}"#;
    assert!(
        RowSeed(limits())
            .deserialize(&mut serde_json::Deserializer::from_slice(missing))
            .is_err(),
        "an absent nullable marker is not an explicit unknown"
    );
}

#[test]
fn field_row_shared_seed_rejects_duplicate_refs_and_false_incomplete() {
    for origin in [
        r#""containsEncryptedSource":false,"sourceUsesRefs":[],"sourceSetComplete":false"#,
        r#""containsEncryptedSource":true,"sourceUsesRefs":[],"sourceSetComplete":true"#,
        r#""containsEncryptedSource":null,"sourceUsesRefs":["same","same"],"sourceSetComplete":false"#,
        r#""containsEncryptedSource":true,"sourceUsesRefs":[""],"sourceSetComplete":false"#,
    ] {
        let line = format!(
            r#"{{"type":"field","scope":{{"stream":{{"type":"sources"}},"ordinal":"0"}},"field":"/data",{origin}}}"#
        );
        assert!(
            RowSeed(limits())
                .deserialize(&mut serde_json::Deserializer::from_str(&line))
                .is_err(),
            "shared correlation/ref policy rejects {origin}"
        );
    }
}

#[test]
fn field_row_known_variant_fields_cannot_be_smuggled_into_a_header() {
    let line = br#"{"type":"occurrence","scope":{"stream":{"type":"sources"},"ordinal":"0"},"profile":"unmigrated","payload_sha256":"digest","field":"/data"}"#;
    assert!(
        RowSeed(limits())
            .deserialize(&mut serde_json::Deserializer::from_slice(line))
            .is_err(),
        "a closed header cannot also claim a field row"
    );
}

#[test]
fn field_row_enforces_explicit_consumer_reference_policy() {
    let line = br#"{"type":"field","scope":{"stream":{"type":"sources"},"ordinal":"0"},"field":"/data","containsEncryptedSource":true,"sourceUsesRefs":["one","two"],"sourceSetComplete":false}"#;
    let mut policy = limits();
    assert!(
        RowSeed(policy)
            .deserialize(&mut serde_json::Deserializer::from_slice(line))
            .is_ok(),
        "producer-independent reader allowance admits both refs"
    );
    policy.field_uses = 1;
    assert!(
        RowSeed(policy)
            .deserialize(&mut serde_json::Deserializer::from_slice(line))
            .is_err(),
        "the explicit consumer may choose a smaller allowance"
    );
}
