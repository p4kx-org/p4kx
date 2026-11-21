// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::de::DeserializeSeed as _;

use crate::origin_reader_tests::reader_limits;
use crate::{FieldOrigin, FieldOriginsSeed, FieldPath, OriginReaderLimits};

#[test]
fn unknown_map_length_refuses_unadmitted_keys_before_path_and_metadata_policies() {
    let limits = OriginReaderLimits {
        fields: 0,
        metadata_bytes: 0,
        path_bytes: 0,
        ..reader_limits()
    };
    let input = r#"{"invalid-pointer":{"containsEncryptedSource":false,"sourceUsesRefs":[],"sourceSetComplete":true}}"#;
    let result =
        FieldOriginsSeed::new(limits).deserialize(&mut serde_json::Deserializer::from_str(input));
    let error = result.expect_err("the first field is not admitted");
    assert!(
        error
            .to_string()
            .contains("field count exceeds the reader policy"),
        "unknown-size JSON maps enforce field admission before competing path syntax, path bytes or metadata bytes: {error}"
    );
}

#[test]
fn the_key_after_the_admitted_map_boundary_is_not_copied_or_validated() {
    let one_field_bytes = 2 + size_of::<FieldPath>() + size_of::<FieldOrigin>();
    let limits = OriginReaderLimits {
        fields: 1,
        path_bytes: 2,
        metadata_bytes: one_field_bytes,
        ..reader_limits()
    };
    let input = r#"{"/a":{"containsEncryptedSource":false,"sourceUsesRefs":[],"sourceSetComplete":true},"invalid-pointer":{"containsEncryptedSource":false,"sourceUsesRefs":[],"sourceSetComplete":true}}"#;
    let result =
        FieldOriginsSeed::new(limits).deserialize(&mut serde_json::Deserializer::from_str(input));
    let error = result.expect_err("the second field is not admitted");
    assert!(
        error
            .to_string()
            .contains("field count exceeds the reader policy"),
        "the next unadmitted key cannot spend or reset the shared metadata budget: {error}"
    );
}
