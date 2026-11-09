// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::ConflictPolicy;
use p4kx_testkit::{TestContainer, TestEntry};
use serde_json::Value;

use crate::origin_test_support::{self as support, TestResult};

const SHADER: &[u8] = b"Version(1)\nProperty { Name(%A); Mask(0x10); }";

#[test]
fn shader_fields_distinguish_directory_metadata_payload_and_encrypted_ancestors() -> TestResult {
    let folder = tempfile::tempdir()?;
    let nested = TestContainer::new()
        .entry(TestEntry::stored("nested.ext", SHADER))
        .build();
    let (_, output, index) = support::export(
        folder.path(),
        [
            TestEntry::stored("plain.ext", SHADER),
            TestEntry::zstd("encrypted.ext", SHADER).encrypted(),
            TestEntry::zstd("container.socpak", nested).encrypted(),
        ],
    )?;
    for (path, metadata, payload) in [
        ("plain.ext", false, false),
        ("encrypted.ext", false, true),
        ("nested.ext", true, true),
    ] {
        let ordinal = support::ordinal(&output, &index, "shader", path, "shader-definition")?;
        let file_name = support::field(
            &output,
            &index,
            "shader",
            ordinal,
            "/data/payload/file_name",
        )?;
        let encoding =
            support::field(&output, &index, "shader", ordinal, "/data/payload/encoding")?;
        assert_eq!(
            support::marker(&file_name),
            Some(metadata),
            "{path}: directory domain"
        );
        assert_eq!(
            support::marker(&encoding),
            Some(payload),
            "{path}: actual payload domain"
        );
        assert_eq!(support::complete(&file_name), Some(true));
        assert_eq!(support::complete(&encoding), Some(true));
    }
    assert_source_domains(&output, &index)?;
    Ok(())
}

fn assert_source_domains(output: &std::path::Path, index: &crate::Index) -> TestResult {
    let rows = support::rows(output, index, "origin-uses.jsonl")?;
    assert!(
        rows.iter().any(|row| {
            row.get("scope").and_then(Value::as_str) == Some("archiveDirectoryMetadata")
                && row.get("effectiveEncryption").and_then(Value::as_str) == Some("unencrypted")
        }),
        "the original directory use is explicitly plain"
    );
    assert!(
        rows.iter().any(|row| {
            row.get("scope").and_then(Value::as_str) == Some("payload")
                && row.get("effectiveEncryption").and_then(Value::as_str) == Some("encrypted")
        }),
        "the original payload use retains the encrypted witness"
    );
    assert!(
        rows.iter()
            .any(|row| row.get("role").and_then(Value::as_str) == Some("conversion")),
        "derived names, counts and hashes retain conversion uses"
    );
    Ok(())
}

#[test]
fn equal_raw_artifacts_keep_distinct_plain_and_encrypted_occurrence_origins() -> TestResult {
    let folder = tempfile::tempdir()?;
    let (_, output, index) = support::export(
        folder.path(),
        [
            TestEntry::stored("plain.ext", SHADER),
            TestEntry::zstd("encrypted.ext", SHADER).encrypted(),
        ],
    )?;
    let artifact_rows = support::rows(&output, &index, "artifacts.jsonl")?;
    let raw: Vec<_> = artifact_rows
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            row.get("path")
                .and_then(Value::as_str)
                .is_some_and(|path| path.starts_with("raw/"))
        })
        .collect();
    assert_eq!(
        raw.len(),
        2,
        "raw manifest preserves both actual source occurrences"
    );
    assert_eq!(
        raw.first().map(|(_, row)| row.get("path")),
        raw.get(1).map(|(_, row)| row.get("path")),
        "decoded bytes share one file"
    );
    let fields = support::rows(&output, &index, "field-origins.jsonl")?;
    for ((ordinal, _), expected) in raw.into_iter().zip([false, true]) {
        let ordinal = ordinal.to_string();
        let digest = fields
            .iter()
            .find(|row| {
                row.get("type").and_then(Value::as_str) == Some("field")
                    && row.pointer("/scope/stream/type").and_then(Value::as_str)
                        == Some("artifacts")
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(ordinal.as_str())
                    && row.get("field").and_then(Value::as_str) == Some("/digest/hex")
            })
            .ok_or("artifact occurrence intrinsic digest")?;
        assert_eq!(support::marker(digest), Some(expected));
    }
    Ok(())
}

#[test]
fn provenance_transport_does_not_spend_the_original_payload_output_budget() -> TestResult {
    let folder = tempfile::tempdir()?;
    let (archive, first_output, index) =
        support::export(folder.path(), [TestEntry::stored("plain.ext", SHADER)])?;
    let payload_bytes = payload_bytes(&first_output, &index)?;
    let receipts = index
        .field_origins
        .as_ref()
        .ok_or("required current origins")?;
    assert!(receipts.fields.size > 0 && receipts.sources.size > 0 && receipts.uses.size > 0);
    let options = crate::ExportOptions {
        limits: crate::Limits {
            output_bytes: payload_bytes,
            ..crate::Limits::default()
        },
        conflict: ConflictPolicy::Overwrite,
        ..crate::ExportOptions::default()
    };
    let registry = crate::ProviderRegistry::try_new(vec![
        Box::new(crate::ArchiveProvider),
        Box::new(crate::ShaderProvider::default()),
    ])?;
    let outcome = crate::export_with(
        &archive,
        &folder.path().join("exact-budget"),
        &options,
        &registry,
    )?;
    assert!(
        matches!(outcome, crate::ExportOutcome::Published { .. }),
        "all additional origin bytes are outside the unchanged payload budget"
    );
    let below = crate::ExportOptions {
        limits: crate::Limits {
            output_bytes: payload_bytes - 1,
            ..crate::Limits::default()
        },
        conflict: ConflictPolicy::Overwrite,
        ..crate::ExportOptions::default()
    };
    let destination = folder.path().join("below-original-budget");
    let failure = crate::export_with(&archive, &destination, &below, &registry)
        .err()
        .ok_or("one byte below the original output budget must fail")?;
    assert!(
        support::has_limit(&failure, "output_bytes"),
        "the original output boundary remains exact"
    );
    assert!(!destination.join("media-index.json").exists());
    Ok(())
}

fn payload_bytes(
    output: &std::path::Path,
    index: &crate::Index,
) -> Result<u64, Box<dyn std::error::Error>> {
    let artifacts = support::rows(output, index, "artifacts.jsonl")?;
    let original_files = index.source_manifest.size
        + index.artifact_manifest.size
        + index
            .kinds
            .iter()
            .map(|kind| kind.artifact.size)
            .sum::<u64>();
    let retained_bytes = artifacts
        .iter()
        .map(|row| {
            row.get("size")
                .and_then(Value::as_u64)
                .ok_or("actual artifact length")
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum::<u64>();
    let raw = support::rows(output, index, "sources.jsonl")?
        .into_iter()
        .map(|row| {
            row.pointer("/raw/path")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or("retained raw path")
        })
        .collect::<Result<Vec<_>, _>>()?;
    for path in raw {
        assert_eq!(
            fs::read(output.join(&index.artifact_base).join(path))?,
            SHADER
        );
    }
    Ok(original_files + retained_bytes)
}
#[test]
fn excluded_encrypted_source_is_not_read_and_cannot_taint_the_selected_plain_shader() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    p4kx_testkit::ArchiveBuilder::new()
        .entry(TestEntry::stored("plain.ext", SHADER))
        .entry(TestEntry::zstd("unread.ext", SHADER).encrypted())
        .write_to(&path)?;
    let archive = p4kx_extraction::Archive::open_with_key(path, None)?;
    let output = folder.path().join("media");
    let options = crate::ExportOptions {
        exclude: p4kx_extraction::Exclusion::parse(["unread.ext"])?,
        ..crate::ExportOptions::default()
    };
    let registry =
        crate::ProviderRegistry::try_new(vec![Box::new(crate::ShaderProvider::default())])?;
    let crate::ExportOutcome::Published { index, .. } =
        crate::export_with(&archive, &output, &options, &registry)?
    else {
        return Err("selected plain source remains complete without a key".into());
    };
    let ordinal = support::ordinal(&output, &index, "shader", "plain.ext", "shader-definition")?;
    let encoding = support::field(&output, &index, "shader", ordinal, "/data/payload/encoding")?;
    assert_eq!(support::marker(&encoding), Some(false));
    assert_eq!(support::complete(&encoding), Some(true));
    assert_eq!(support::rows(&output, &index, "sources.jsonl")?.len(), 1);
    assert_eq!(index.source_entries.skipped, 1);
    assert!(
        support::rows(&output, &index, "origin-sources.jsonl")?
            .iter()
            .all(
                |source| source.get("storedEncryption").and_then(Value::as_str)
                    != Some("encrypted")
            ),
        "the untouched encrypted target contributes no physical payload source"
    );
    Ok(())
}
#[test]
fn common_source_intrinsics_reuse_original_sites_across_different_shader_subresources() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let (_, output, index) =
        support::export(folder.path(), [TestEntry::stored("plain.ext", SHADER)])?;
    let definition = support::ordinal(&output, &index, "shader", "plain.ext", "shader-definition")?;
    let declaration =
        support::ordinal(&output, &index, "shader", "plain.ext", "shader-declaration")?;
    for pointer in ["/source/raw/digest/hex", "/source/root/hex"] {
        let first = support::field(&output, &index, "shader", definition, pointer)?;
        let second = support::field(&output, &index, "shader", declaration, pointer)?;
        assert_eq!(
            first.get("sourceUsesRefs"),
            second.get("sourceUsesRefs"),
            "one intrinsic operation does not acquire output-declaration coordinates"
        );
    }
    Ok(())
}
