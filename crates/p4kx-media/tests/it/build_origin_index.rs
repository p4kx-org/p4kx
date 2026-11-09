// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Write as _;

use p4kx_media::{ExportOutcome, MediaError};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::build_origin_fixture::{self as fixture, Channel, FIELDS, Input, TestResult};
use super::build_origin_support as check;

#[test]
fn index_build_and_archive_root_share_original_uses_with_separate_exact_occurrences() -> TestResult
{
    let bytes = fixture::manifest("paired")?;
    let channel = Channel::new(Input::File(&bytes), false)?;
    let run = channel.export("media", &fixture::options(), &fixture::registry()?)?;
    check_pairs(&run)?;
    check_projection(&run, "6")?;
    super::build_origin_tamper::unknown_build_member(&channel, &run)?;
    super::build_origin_tamper::rebound_scopes(&channel)?;
    super::build_origin_tamper::historical_missing_scope(&channel)?;
    check_without_archive_provider(&channel)
}

#[test]
fn null_index_build_has_one_actual_node_and_its_receipt_is_required_on_reuse() -> TestResult {
    let channel = Channel::new(Input::Missing, false)?;
    let registry = fixture::registry()?;
    let options = fixture::options();
    let run = channel.export("media", &options, &registry)?;
    assert!(run.index.source.build.is_none());
    check::unknown(&run, "indexBuild", "/source/build")?;
    check_projection(&run, "1")?;
    let paths: Vec<_> = run
        .fields
        .iter()
        .filter(|row| {
            row.get("type").and_then(Value::as_str) == Some("field")
                && row.pointer("/scope/stream/type").and_then(Value::as_str) == Some("indexBuild")
        })
        .map(|row| row.get("field").and_then(Value::as_str))
        .collect();
    assert_eq!(
        paths,
        [Some("/source/build")],
        "a null Build has no invented child addresses"
    );
    let committed = fs::read(run.output.join("media-index.json"))?;
    let ExportOutcome::Skipped { .. } =
        p4kx_media::export_with(&channel.archive, &run.output, &options, &registry)?
    else {
        return Err("validated current Build scope was not reused".into());
    };
    fs::OpenOptions::new()
        .append(true)
        .open(run.base.join("field-origins.jsonl"))?
        .write_all(b"\n")?;
    let failure = p4kx_media::export_with(&channel.archive, &run.output, &options, &registry)
        .err()
        .ok_or("modified declared companion was reused")?;
    assert!(
        super::converted_artifact_support::contains(&failure, |cause| matches!(
            cause,
            MediaError::Destination(_)
        )),
        "actual origin receipt corruption is a destination refusal: {failure}"
    );
    assert_eq!(fs::read(run.output.join("media-index.json"))?, committed);
    Ok(())
}

#[test]
fn build_origin_transport_keeps_payload_bytes_and_the_exact_original_output_budget() -> TestResult {
    let bytes = fixture::manifest("budget")?;
    let channel = Channel::new(Input::File(&bytes), false)?;
    let registry = fixture::registry()?;
    let original = channel.export("original", &fixture::options(), &registry)?;
    let payloads = check::payloads(&original)?;
    let mut bound = 0_u64;
    for bytes in payloads.values() {
        bound = bound
            .checked_add(u64::try_from(bytes.len())?)
            .ok_or("payload length overflow")?;
    }
    let below = bound
        .checked_sub(1)
        .ok_or("nonempty original payload set")?;
    let mut options = fixture::options();
    options.limits.output_bytes = bound;
    let exact = channel.export("exact", &options, &registry)?;
    assert_eq!(
        check::payloads(&exact)?,
        payloads,
        "origin-only Build coverage leaves all admitted payload bytes unchanged"
    );
    options.limits.output_bytes = below;
    let failure = p4kx_media::export_with(
        &channel.archive,
        &channel.folder.path().join("below"),
        &options,
        &registry,
    )
    .err()
    .ok_or("payload set one byte above output_bytes was admitted")?;
    assert!(
        super::converted_artifact_support::contains(
            &failure,
            |cause| matches!(cause, MediaError::Limit { budget, limit } if *budget == "output_bytes" && *limit == below)
        ),
        "one-byte-smaller payload cap fails at its owning output budget: {failure}"
    );
    check::file_field(&exact, "records", "/data/build/branch", Some("Data.Branch"))?;
    check::file_field(
        &exact,
        "indexBuild",
        "/source/build/branch",
        Some("Data.Branch"),
    )
}

fn check_pairs(run: &fixture::Run) -> TestResult {
    for (field, logical) in FIELDS {
        let record = format!("/data/build/{field}");
        let index = format!("/source/build/{field}");
        check::file_field(run, "records", &record, Some(logical))?;
        check::file_field(run, "indexBuild", &index, Some(logical))?;
        assert_eq!(
            fixture::references(run.field("records", &record)?)?,
            fixture::references(run.field("indexBuild", &index)?)?,
            "both placements reuse the same original logical source uses"
        );
    }
    check::independent(run, "indexBuild", "/source/build/public_patch")?;
    check_aggregate(run)?;
    Ok(())
}

fn check_projection(run: &fixture::Run, fields: &str) -> TestResult {
    let occurrences: Vec<_> = run
        .fields
        .iter()
        .filter(|row| {
            row.get("type").and_then(Value::as_str) == Some("occurrence")
                && row.pointer("/scope/stream/type").and_then(Value::as_str) == Some("indexBuild")
        })
        .collect();
    assert_eq!(
        occurrences.len(),
        1,
        "Index Build has exactly its own one occurrence"
    );
    let occurrence = occurrences
        .first()
        .ok_or("asserted Index Build occurrence")?;
    assert_eq!(
        occurrence.pointer("/scope/ordinal").and_then(Value::as_str),
        Some("0"),
        "the single Index Build occurrence uses its original zero ordinal"
    );
    let projection = serde_json::to_vec(&run.index.source.build)?;
    let expected = p4kx_extraction::Checksum::Sha256(Sha256::digest(&projection).into()).hex();
    assert_eq!(
        occurrence.get("payload_sha256").and_then(Value::as_str),
        Some(expected.as_str()),
        "the hash covers compact nullable Build only, without a newline or Index receipts"
    );
    check_projection_summary(run, fields)
}

fn check_projection_summary(run: &fixture::Run, fields: &str) -> TestResult {
    let receipt = serde_json::to_value(
        run.index
            .field_origins
            .as_ref()
            .ok_or("actual origin receipts")?,
    )?;
    let streams = receipt
        .get("streams")
        .and_then(Value::as_array)
        .ok_or("actual stream summaries")?;
    let summaries: Vec<_> = streams
        .iter()
        .filter(|summary| {
            summary.pointer("/stream/type").and_then(Value::as_str) == Some("indexBuild")
        })
        .collect();
    assert_eq!(
        summaries.len(),
        1,
        "Index Build has exactly one stream receipt summary"
    );
    let summary = summaries.first().ok_or("asserted Index Build summary")?;
    assert_eq!(
        summary.get("occurrences").and_then(Value::as_str),
        Some("1"),
        "the receipt counts exactly its one actual nullable Build projection"
    );
    assert_eq!(
        summary.get("fields").and_then(Value::as_str),
        Some(fields),
        "each actual nullable Build node has exactly one immediate origin"
    );
    Ok(())
}

fn check_aggregate(run: &fixture::Run) -> TestResult {
    check::file_field(run, "records", "/data/build", None)?;
    check::file_field(run, "indexBuild", "/source/build", None)?;
    let record = fixture::references(run.field("records", "/data/build")?)?;
    assert_eq!(
        record,
        fixture::references(run.field("indexBuild", "/source/build")?)?,
        "both aggregate placements retain the same original contributing use set"
    );
    for (field, _) in FIELDS {
        for reference in
            fixture::references(run.field("records", &format!("/data/build/{field}"))?)?
        {
            assert!(
                record.contains(reference),
                "the aggregate retains every actual child contribution"
            );
        }
    }
    Ok(())
}
fn check_without_archive_provider(channel: &Channel) -> TestResult {
    let mut options = fixture::options();
    options.kinds = vec![p4kx_media::MediaKind::Material];
    let registry = p4kx_media::ProviderRegistry::try_new(vec![Box::new(
        p4kx_media::MaterialProvider::default(),
    )])?;
    let run = channel.export("without-archive", &options, &registry)?;
    assert_eq!(
        run.index.kinds.len(),
        1,
        "the fixture selects only the actual Material provider"
    );
    assert_eq!(
        run.index
            .kinds
            .first()
            .ok_or("actual selected Material kind")?
            .provider
            .kind,
        p4kx_media::MediaKind::Material,
        "Build indexing must work without an Archive provider callback"
    );
    let expected: &[Value] = &[];
    assert_eq!(
        run.records.as_slice(),
        expected,
        "an unselected Archive provider emits no root records"
    );
    assert!(
        !run.base.join("archive.jsonl").exists(),
        "no Archive kind/provider emitted the Build source uses"
    );
    assert!(
        run.index.source.build.is_some(),
        "the same original Some Build still belongs to the provider-independent Index"
    );
    for (field, logical) in FIELDS {
        check::file_field(
            &run,
            "indexBuild",
            &format!("/source/build/{field}"),
            Some(logical),
        )?;
    }
    check::independent(&run, "indexBuild", "/source/build/public_patch")?;
    check_projection(&run, "6")
}
