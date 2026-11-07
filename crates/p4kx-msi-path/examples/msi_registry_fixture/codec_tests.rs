// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_msi_path::native_fixture::FixtureValue;

use super::Value;

#[test]
fn raw_string_kinds_preserve_empty_missing_single_and_repeated_terminator_bytes() {
    for kind in [1, 2] {
        for bytes in [
            vec![],
            vec![b'A', 0],
            vec![b'A', 0, 0, 0],
            vec![b'A', 0, 0, 0, 0, 0],
            vec![0, 0],
            vec![0, 0, 0, 0],
            vec![0, 0xd8],
        ] {
            let raw = FixtureValue::new(kind, bytes.clone()).expect("bounded raw fixture");
            let decoded = Value::from_raw(&raw).into_raw().expect("raw round trip");
            assert_eq!(decoded.kind(), kind);
            assert_eq!(decoded.bytes(), bytes);
        }
    }
}

#[test]
fn alternate_hex_unknown_fields_and_oversized_lengths_are_not_canonical_evidence() {
    for text in [
        r#"{"registry_type":1,"bytes_hex":"0"}"#,
        r#"{"registry_type":1,"bytes_hex":"AF"}"#,
        r#"{"registry_type":1,"bytes_hex":"gg"}"#,
    ] {
        let value: Value = serde_json::from_str(text).expect("DTO shape");
        assert!(value.into_raw().is_err());
    }
    assert!(
        serde_json::from_str::<Value>(r#"{"registry_type":1,"bytes_hex":"","extra":true}"#)
            .is_err()
    );
    assert!(
        Value {
            registry_type: 12,
            bytes_hex: String::new()
        }
        .into_raw()
        .is_err()
    );
    assert!(
        Value {
            registry_type: 1,
            bytes_hex: "0".repeat(super::MAX_RAW_BYTES * 2 + 1)
        }
        .into_raw()
        .is_err()
    );
}
