// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::MediaError;

#[test]
fn json_nodes_keep_empty_escaped_and_list_addresses() -> Result<(), MediaError> {
    let bytes = br#"{"":[null,{"a/b":{"~":[]}}],"empty":{},"flag":true}"#;
    let mut paths = Vec::new();
    super::visit(bytes, 8, &mut |path| {
        paths.push(path.to_owned());
        Ok(())
    })?;
    assert_eq!(
        paths,
        [
            "",
            "/",
            "//0",
            "//1",
            "//1/a~1b",
            "//1/a~1b/~0",
            "/empty",
            "/flag"
        ],
        "every container and scalar keeps its actual absolute address"
    );
    Ok(())
}

#[test]
fn json_visit_preserves_original_callback_error() {
    let error = super::visit(br#"{"value":1}"#, 8, &mut |path| {
        if path == "/value" {
            Err(MediaError::Cancelled)
        } else {
            Ok(())
        }
    })
    .expect_err("the callback stops the walk");
    assert!(
        matches!(error, MediaError::Cancelled),
        "original error retained"
    );
}

#[test]
fn json_visit_accepts_existing_deep_payload_admission() -> Result<(), MediaError> {
    let depth = 200;
    let mut bytes = vec![b'['; depth];
    bytes.push(b'0');
    bytes.extend(std::iter::repeat_n(b']', depth));
    let mut observed = 0;
    super::visit(&bytes, 512, &mut |_| {
        observed += 1;
        Ok(())
    })?;
    assert_eq!(observed, depth + 1, "deep existing payload stays admitted");
    assert!(
        super::visit(&bytes, 128, &mut |_| Ok(())).is_err(),
        "the configured owning depth still applies"
    );
    Ok(())
}

#[test]
fn decimal_reader_requires_canonical_string() {
    #[derive(serde::Deserialize)]
    struct Value {
        #[serde(deserialize_with = "crate::origin_decimal::deserialize")]
        ordinal: p4kx_extraction::DecimalU64,
    }
    let maximum: Value = serde_json::from_str(r#"{"ordinal":"18446744073709551615"}"#)
        .expect("the full unsigned range is retained");
    assert_eq!(
        maximum.ordinal.get(),
        u64::MAX,
        "no floating-point conversion"
    );
    for invalid in [
        r#"{"ordinal":1}"#,
        r#"{"ordinal":"01"}"#,
        r#"{"ordinal":""}"#,
        r#"{"ordinal":"+1"}"#,
        r#"{"ordinal":"18446744073709551616"}"#,
    ] {
        assert!(
            serde_json::from_str::<Value>(invalid).is_err(),
            "reject {invalid}"
        );
    }
}
