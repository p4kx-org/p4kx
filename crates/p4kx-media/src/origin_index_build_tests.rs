// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::origin_contract::{
    OriginProtocol, OriginReceipts, OriginScope, OriginStream, OriginStreamSummary,
};
use crate::origin_index_build_read::IndexBuildPayload;
use crate::{BuildProvenance, Index, MediaError};

use crate::origin_index_test_support as support;
use support::TestResult;

const CANONICAL: &[u8] =
    br#"{"branch":"b","version":"v","change_number":"c","build_id":"i","public_patch":null}"#;

#[test]
fn actual_build_node_shape_is_required_only_for_its_declared_projection_scope() -> TestResult {
    let mut missing_patch: Value = serde_json::from_slice(CANONICAL)?;
    missing_patch
        .as_object_mut()
        .ok_or("Build object")?
        .remove("public_patch");
    let mut unknown: Value = serde_json::from_slice(CANONICAL)?;
    unknown
        .as_object_mut()
        .ok_or("Build object")?
        .insert("unknown_source_datum".into(), json!({"retained": "extra"}));
    support::run(|_, checkpoint| {
        for value in [
            None,
            Some(missing_patch),
            Some(unknown),
            Some(json!(["b", "v", "c", "i", null])),
        ] {
            let declared = index(value.clone(), true)?;
            assert!(
                matches!(
                    IndexBuildPayload::open(&declared, checkpoint),
                    Err(MediaError::Destination(_))
                ),
                "a declared canonical object/null cannot hide missing or other actual nodes"
            );
            let historical = index(value, false)?;
            assert!(
                IndexBuildPayload::open(&historical, checkpoint)?.is_none(),
                "a historical absent projection remains unknown without new shape rejection"
            );
        }
        Ok(())
    })
}

#[test]
fn null_and_object_projection_hashes_bind_their_exact_canonical_bytes_and_node_counts() -> TestResult
{
    support::run(|_, checkpoint| {
        for (expected, count) in [(b"null".as_slice(), 1), (CANONICAL, 6)] {
            let value = serde_json::from_slice(expected)?;
            let index = index(Some(value), true)?;
            let mut payload =
                IndexBuildPayload::open(&index, checkpoint)?.ok_or("declared projection")?;
            let expected_hash =
                p4kx_extraction::Checksum::Sha256(Sha256::digest(expected).into()).hex();
            let scope = OriginScope {
                stream: OriginStream::IndexBuild,
                ordinal: DecimalU64::new(0),
            };
            assert_eq!(
                payload.occurrence(&scope, &expected_hash)?,
                expected,
                "the one projection returns exactly the canonical object/null bytes used by its hash"
            );
            payload.fields(count)?;
            payload.finish(&[summary(count)])?;
            assert!(
                payload.occurrence(&scope, &expected_hash).is_err(),
                "the one actual projection cannot be replayed"
            );
        }
        Ok(())
    })
}

#[test]
fn legacy_patch_defaults_and_unknown_members_keep_public_value_serialization() -> TestResult {
    let expected: Value = serde_json::from_slice(CANONICAL)?;
    for extra in [false, true] {
        let mut value = expected.clone();
        value
            .as_object_mut()
            .ok_or("Build object")?
            .remove("public_patch");
        if extra {
            value
                .as_object_mut()
                .ok_or("Build object")?
                .insert("extra".into(), json!([{"arbitrary": true}]));
        }
        let historical = index(Some(value), false)?;
        assert!(
            historical.source.build_origin.is_none(),
            "JSON never restores original runtime authority"
        );
        assert_eq!(
            serde_json::to_value(historical.source.build)?,
            expected,
            "legacy missing patch and ignored members preserve public Build value serialization"
        );
    }
    let value = serde_json::from_str::<BuildProvenance>(r#"["b","v","c","i",null]"#)?;
    assert_eq!(
        serde_json::to_value(value)?,
        expected,
        "existing typed sequence compatibility stays outside canonical scope"
    );
    Ok(())
}

#[test]
fn known_build_members_preserve_duplicate_and_string_type_refusals() {
    for bytes in [
        br#"{"branch":null,"version":"v","change_number":"c","build_id":"i","public_patch":null}"#.as_slice(),
        br#"{"branch":"b","branch":"c","version":"v","change_number":"c","build_id":"i","public_patch":null}"#,
        br#"{"branch":"b","version":"v","change_number":"c","build_id":"i","public_patch":null,"public_patch":null}"#,
    ] {
        assert!(serde_json::from_slice::<BuildProvenance>(bytes).is_err(),
            "known Build members retain duplicate and string-type refusals: {bytes:?}");
    }
}

fn index(build: Option<Value>, declared: bool) -> Result<Index, Box<dyn std::error::Error>> {
    let mut value = serde_json::to_value(support::index())?;
    let source = value
        .get_mut("source")
        .and_then(Value::as_object_mut)
        .ok_or("Source object")?;
    match build {
        Some(build) => {
            source.insert("build".into(), build);
        }
        None => {
            source.remove("build");
        }
    }
    let mut index: Index = serde_json::from_value(value)?;
    if declared {
        index.field_origins = Some(OriginReceipts {
            protocol: OriginProtocol::V1,
            sources: support::artifact("origin-sources.jsonl"),
            uses: support::artifact("origin-uses.jsonl"),
            fields: support::artifact("field-origins.jsonl"),
            streams: vec![summary(0)],
        });
    }
    Ok(index)
}

const fn summary(fields: u64) -> OriginStreamSummary {
    OriginStreamSummary {
        stream: OriginStream::IndexBuild,
        occurrences: DecimalU64::new(1),
        fields: DecimalU64::new(fields),
    }
}
