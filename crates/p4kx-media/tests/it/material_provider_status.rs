// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Checksum;
use p4kx_extraction::model::MaterialSourceLimits;
use p4kx_media::{MaterialProvider, Status};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use serde_json::Value;
use sha2::Digest as _;

use super::material_support::{self as support, TestResult};

#[test]
fn structured_depth_loss_refuses_normal_publication_and_is_explicitly_partial() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = b"<Material><PublicParams Root='keep'/></Material>";
    let archive = support::archive(folder.path(), [TestEntry::stored("source.mtl", source)])?;
    let mut limits = MaterialSourceLimits::default();
    limits.depth = 0;
    let registry = support::registry(MaterialProvider::new_with_limits(limits))?;
    let output = folder.path().join("out");
    let error = p4kx_media::export_with(&archive, &output, &support::options(), &registry)
        .err()
        .ok_or("partial source published normally")?;
    assert!(
        support::projection_loss(&error),
        "actual source loss follows the common typed publication gate"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "normal publication leaves no complete index"
    );
    let options = p4kx_media::ExportOptions {
        diagnostic: true,
        ..support::options()
    };
    let records = support::diagnostic_records(p4kx_media::export_with(
        &archive, &output, &options, &registry,
    )?)?;
    let record = records.first().ok_or("partial record")?;
    assert_eq!(
        record.status,
        Status::Partial,
        "retained original bytes do not imply full structured metadata"
    );
    assert_eq!(
        record
            .data
            .pointer("/payload/projection_complete")
            .and_then(Value::as_bool),
        Some(false),
        "coverage flag agrees with status"
    );
    assert_eq!(
        record
            .data
            .pointer("/payload/authored/losses/0/kind")
            .and_then(Value::as_str),
        Some("depth"),
        "actual source boundary is named"
    );
    Ok(())
}

#[test]
fn plain_encoding_failure_keeps_exact_raw_digest_and_reports_failed_projection() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = b"<Material Name='invalid\xff'/>";
    let archive = support::archive(folder.path(), [TestEntry::stored("invalid.mtl", source)])?;
    let options = p4kx_media::ExportOptions {
        diagnostic: true,
        ..support::options()
    };
    let records = support::diagnostic_records(p4kx_media::export_with(
        &archive,
        &folder.path().join("out"),
        &options,
        &support::registry(MaterialProvider::default())?,
    )?)?;
    let record = records.first().ok_or("failed record")?;
    assert_eq!(
        record.status,
        Status::Failed,
        "invalid UTF8 is not rewritten into a successful source"
    );
    assert_eq!(
        record
            .data
            .pointer("/payload/reason/type")
            .and_then(Value::as_str),
        Some("text-encoding"),
        "failure is encoding-specific"
    );
    assert_eq!(
        record.source.raw.as_ref().ok_or("raw source")?.digest.hex,
        Checksum::Sha256(sha2::Sha256::digest(source).into()).hex(),
        "exact undecodable original bytes remain independently identified"
    );
    Ok(())
}

#[test]
fn actual_binary_string_repairs_are_partial_but_valid_escaped_strings_are_complete() -> TestResult {
    let folder = tempfile::tempdir()?;
    let root = TestElement::new("Material").attribute("Name", "repair-marker & <quoted>");
    let valid = root.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let mut repaired = valid.clone();
    let marker = b"repair-marker";
    let offset = repaired
        .windows(marker.len())
        .position(|window| window == marker)
        .ok_or("synthetic marker")?;
    *repaired.get_mut(offset).ok_or("marker byte")? = 0xff;
    let archive = support::archive(
        folder.path(),
        [
            TestEntry::stored("valid.mtl", valid.as_slice()),
            TestEntry::stored("repaired.mtl", repaired.as_slice()),
        ],
    )?;
    let options = p4kx_media::ExportOptions {
        diagnostic: true,
        ..support::options()
    };
    let records = support::diagnostic_records(p4kx_media::export_with(
        &archive,
        &folder.path().join("out"),
        &options,
        &support::registry(MaterialProvider::default())?,
    )?)?;
    assert_eq!(
        records.first().ok_or("valid record")?.status,
        Status::Complete,
        "normal binary decoding/escaping does not invent loss"
    );
    let broken = records.get(1).ok_or("repaired record")?;
    assert_eq!(
        broken.status,
        Status::Partial,
        "actual repaired bytes have explicit coverage loss"
    );
    assert_eq!(
        broken
            .data
            .pointer("/payload/authored/losses/0/kind")
            .and_then(Value::as_str),
        Some("repaired-encoding"),
        "actual repair flag survives export"
    );
    assert_eq!(
        broken.source.raw.as_ref().ok_or("raw")?.digest.hex,
        Checksum::Sha256(sha2::Sha256::digest(&repaired).into()).hex(),
        "raw digest belongs to original bytes, not repaired XML"
    );
    Ok(())
}

#[test]
fn material_input_budget_is_a_failed_typed_projection_with_retained_source() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(
        folder.path(),
        [TestEntry::stored("large.mtl", b"<Material/>")],
    )?;
    let mut limits = MaterialSourceLimits::default();
    limits.source_bytes = 1;
    let options = p4kx_media::ExportOptions {
        diagnostic: true,
        ..support::options()
    };
    let records = support::diagnostic_records(p4kx_media::export_with(
        &archive,
        &folder.path().join("out"),
        &options,
        &support::registry(MaterialProvider::new_with_limits(limits))?,
    )?)?;
    let record = records.first().ok_or("bounded source failure")?;
    assert_eq!(
        record.status,
        Status::Failed,
        "structured input bound is not a successful empty library"
    );
    assert_eq!(
        record
            .data
            .pointer("/payload/reason/type")
            .and_then(Value::as_str),
        Some("source-bytes"),
        "failure preserves the exact bound category"
    );
    assert!(
        record.source.raw.is_some(),
        "raw retention is independent of structured parser bounds"
    );
    Ok(())
}
