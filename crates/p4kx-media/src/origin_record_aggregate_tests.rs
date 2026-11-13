// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::TestEntry;
use serde_json::Value;

use crate::origin_aggregate_support::{self as aggregate, TestResult};
use crate::origin_test_support as support;

const SHADER: &[u8] = b"Version(1)\nProperty { Name(%A); Mask(0x10); }";

#[test]
fn encrypted_artifact_and_digest_aggregates_retain_their_known_converted_children() -> TestResult {
    let folder = tempfile::tempdir()?;
    let (_, output, index) = support::export(
        folder.path(),
        [TestEntry::zstd("encrypted.ext", SHADER).encrypted()],
    )?;
    let artifacts = support::rows(&output, &index, "artifacts.jsonl")?;
    let ordinal = artifacts
        .iter()
        .position(|artifact| {
            artifact
                .get("path")
                .and_then(Value::as_str)
                .is_some_and(|path| path.starts_with("raw/"))
        })
        .ok_or("actual retained artifact occurrence")?;
    let root = aggregate::field(&output, &index, "artifacts", ordinal, "")?;
    let path = aggregate::field(&output, &index, "artifacts", ordinal, "/path")?;
    let size = aggregate::field(&output, &index, "artifacts", ordinal, "/size")?;
    let digest = aggregate::field(&output, &index, "artifacts", ordinal, "/digest")?;
    assert_eq!(
        support::marker(&root),
        Some(true),
        "known converted encryption survives object aggregation"
    );
    aggregate::assert_union(&root, &[path, size, digest.clone()])?;
    let mut components = Vec::new();
    for name in ["algorithm", "scope", "hex"] {
        components.push(aggregate::field(
            &output,
            &index,
            "artifacts",
            ordinal,
            &format!("/digest/{name}"),
        )?);
    }
    aggregate::assert_union(&digest, &components)?;
    assert_computed_digest_origins(&components)
}

fn assert_computed_digest_origins(components: &[Value]) -> TestResult {
    let [algorithm, scope, hex] = components else {
        return Err("the three computed digest components".into());
    };
    for field in [algorithm, scope] {
        assert_eq!(
            support::marker(field),
            Some(false),
            "fixed digest descriptor"
        );
        assert_eq!(
            support::complete(field),
            Some(true),
            "known independent value"
        );
        assert!(
            aggregate::references(field)?.is_empty(),
            "no bytes read for the descriptor"
        );
    }
    assert_eq!(
        support::marker(hex),
        Some(true),
        "computed hash uses encrypted bytes"
    );
    assert!(
        !aggregate::references(hex)?.is_empty(),
        "the actual bytes remain bound"
    );
    Ok(())
}

#[test]
fn builtin_record_and_definition_wrappers_keep_known_metadata_payload_and_root_inputs() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let (_, output, index) = support::export(
        folder.path(),
        [TestEntry::zstd("encrypted.ext", SHADER).encrypted()],
    )?;
    let ordinal = support::ordinal(
        &output,
        &index,
        "shader",
        "encrypted.ext",
        "shader-definition",
    )?;
    let filename = support::field(
        &output,
        &index,
        "shader",
        ordinal,
        "/data/payload/file_name",
    )?;
    let encoding = support::field(&output, &index, "shader", ordinal, "/data/payload/encoding")?;
    let counts = support::field(&output, &index, "shader", ordinal, "/data/payload/counts")?;
    let data = support::field(&output, &index, "shader", ordinal, "/data")?;
    let payload = support::field(&output, &index, "shader", ordinal, "/data/payload")?;
    assert_eq!(
        support::marker(&filename),
        Some(false),
        "filename uses public leaf metadata"
    );
    assert_eq!(
        support::marker(&encoding),
        Some(true),
        "parsed encoding uses the encrypted payload"
    );
    aggregate::assert_contains(&data, &[filename.clone(), encoding.clone(), counts.clone()])?;
    aggregate::assert_contains(&payload, &[filename, encoding, counts])?;
    let root = support::field(&output, &index, "shader", ordinal, "")?;
    let source = support::field(&output, &index, "shader", ordinal, "/source")?;
    let subresource = support::field(&output, &index, "shader", ordinal, "/subresource")?;
    assert_eq!(
        support::marker(&root),
        Some(true),
        "known encrypted children survive other unknown record inputs"
    );
    assert_eq!(
        support::complete(&root),
        Some(false),
        "unmigrated issue and other wrapper input sets remain explicitly incomplete"
    );
    aggregate::assert_contains(&root, &[source, data, subresource])?;
    Ok(())
}
