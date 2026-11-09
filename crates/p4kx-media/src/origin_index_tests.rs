// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::Write as _;
use std::path::Path;

use crate::checkpoint::Checkpoint;
use crate::generation_validation;
use crate::hashing;
use crate::index_destination::IndexDestination;
use crate::index_file;
use crate::origin_contract::{OriginReceipts, OriginStream};
use crate::publication::Publication;
use crate::{Algorithm, ByteScope, Index, Limits, MediaError};

use super::{Budget, Buffer, Counter, Encoding};

use crate::origin_index_test_support as support;

use support::TestResult;

fn bytes(index: &Index, encoding: Encoding) -> Result<Vec<u8>, serde_json::Error> {
    match encoding {
        Encoding::Compact => serde_json::to_vec(index),
        Encoding::Pretty => {
            let mut bytes = serde_json::to_vec_pretty(index)?;
            bytes.push(b'\n');
            Ok(bytes)
        }
    }
}

fn exact_old_limit(index: &mut Index, encoding: Encoding) -> TestResult {
    assert!(index.field_origins.is_none());
    loop {
        let actual = u64::try_from(bytes(index, encoding)?.len())?;
        if actual == index.limits.record_bytes {
            return Ok(());
        }
        index.limits.record_bytes = actual;
    }
}

#[test]
fn maximum_closed_group_adds_only_its_actual_compact_or_pretty_insertion() -> TestResult {
    for encoding in [Encoding::Compact, Encoding::Pretty] {
        let mut index = support::index();
        exact_old_limit(&mut index, encoding)?;
        let old = bytes(&index, encoding)?;
        assert_eq!(u64::try_from(old.len())?, index.limits.record_bytes);
        index.field_origins = Some(super::maximum_receipts());
        let full = bytes(&index, encoding)?;
        let budget = Budget::for_index(&index, encoding)?;
        assert_eq!(
            u64::try_from(full.len())?,
            index.limits.record_bytes + budget.control
        );
        let mut writer = Buffer::new(budget);
        writer.write_all(&full)?;
        writer.finish()?;
        assert_eq!(writer.bytes, full);
        index.source.path.push('x');
        let too_large = bytes(&index, encoding)?;
        assert!(budget.finish(u64::try_from(too_large.len())?).is_err());
        assert!(Buffer::new(budget).write_all(&too_large).is_err());
    }
    Ok(())
}

#[test]
fn small_current_group_cannot_borrow_unused_maximum_control_headroom() -> TestResult {
    let mut index = support::index();
    exact_old_limit(&mut index, Encoding::Compact)?;
    let mut receipts = super::maximum_receipts();
    receipts.streams.retain(|summary| {
        matches!(
            summary.stream,
            OriginStream::Sources | OriginStream::Artifacts
        )
    });
    index.field_origins = Some(receipts);
    let budget = Budget::for_index(&index, Encoding::Compact)?;
    assert!(budget.bound < super::read_bound(index.limits.record_bytes)?);
    index.source.path.push('x');
    let actual = u64::try_from(bytes(&index, Encoding::Compact)?.len())?;
    assert!(actual <= super::read_bound(index.limits.record_bytes)?);
    assert!(
        budget.finish(actual).is_err(),
        "old payload growth cannot consume unused maximum allowance"
    );
    assert!(super::validate_read(&index, actual, index.limits.record_bytes, None).is_err());
    Ok(())
}

#[test]
fn bounded_read_admits_both_encodings_and_rejects_one_byte_past_the_envelope() -> TestResult {
    support::run(|root, checkpoint| {
        let path = root.join("index.json");
        let mut index = support::index();
        exact_old_limit(&mut index, Encoding::Pretty)?;
        index.field_origins = Some(super::maximum_receipts());
        for encoding in [Encoding::Compact, Encoding::Pretty] {
            fs::write(&path, bytes(&index, encoding)?)?;
            let decoded = super::read(&path, index.limits.record_bytes, &checkpoint)?;
            assert!(decoded.field_origins.is_some());
        }
        let bound = super::read_bound(index.limits.record_bytes)?;
        let oversize = usize::try_from(bound.checked_add(1).ok_or("test envelope overflow")?)?;
        fs::write(&path, vec![b' '; oversize])?;
        assert!(matches!(
            super::read(&path, index.limits.record_bytes, &checkpoint),
            Err(MediaError::Limit {
                budget: "index_bytes",
                ..
            })
        ));
        Ok(())
    })
}

#[test]
fn historical_none_retains_its_raw_limit_without_a_pretty_reader_requirement() -> TestResult {
    support::run(|root, checkpoint| {
        let path = root.join("historical.json");
        let mut index = support::index();
        exact_old_limit(&mut index, Encoding::Compact)?;
        let mut compact = bytes(&index, Encoding::Compact)?;
        fs::write(&path, &compact)?;
        assert!(
            super::read(&path, index.limits.record_bytes, &checkpoint)?
                .field_origins
                .is_none()
        );
        let pretty = bytes(&index, Encoding::Pretty)?;
        assert!(u64::try_from(pretty.len())? > index.limits.record_bytes);
        assert!(
            Budget::for_index(&index, Encoding::Pretty)?
                .finish(u64::try_from(pretty.len())?)
                .is_err()
        );
        compact.push(b' ');
        fs::write(&path, compact)?;
        assert!(matches!(
            super::read(&path, index.limits.record_bytes, &checkpoint),
            Err(MediaError::Limit {
                budget: "index_bytes",
                ..
            })
        ));
        Ok(())
    })
}

#[test]
fn malformed_or_oversized_control_groups_do_not_receive_an_allowance() -> TestResult {
    for receipts in malformed_receipts()? {
        assert!(super::validate_receipts(&receipts).is_err());
        let mut index = support::index();
        index.field_origins = Some(receipts);
        assert!(Budget::for_index(&index, Encoding::Compact).is_err());
        assert!(super::validate_read(&index, 0, index.limits.record_bytes, None).is_err());
    }
    Ok(())
}

fn malformed_receipts() -> Result<Vec<OriginReceipts>, Box<dyn Error>> {
    let mut malformed = Vec::new();
    let mut receipts = super::maximum_receipts();
    receipts.sources.path.push('x');
    malformed.push(receipts);
    let mut receipts = super::maximum_receipts();
    receipts.uses.digest.hex.push('0');
    malformed.push(receipts);
    let mut receipts = super::maximum_receipts();
    receipts.fields.digest.hex.replace_range(..1, "z");
    malformed.push(receipts);
    let mut receipts = super::maximum_receipts();
    receipts.sources.digest.algorithm = Algorithm::Crc32;
    malformed.push(receipts);
    let mut receipts = super::maximum_receipts();
    receipts.uses.digest.scope = ByteScope::DecodedEntryPayload;
    malformed.push(receipts);
    let mut receipts = super::maximum_receipts();
    receipts.streams.push(receipts.streams[0].clone());
    malformed.push(receipts);
    let mut receipts = super::maximum_receipts();
    receipts
        .streams
        .retain(|summary| summary.stream != OriginStream::Artifacts);
    malformed.push(receipts);
    let mut receipts = super::maximum_receipts();
    receipts.streams[0] = receipts.streams[1].clone();
    malformed.push(receipts);
    let mut receipts = super::maximum_receipts();
    receipts.sources.path = "x".repeat(usize::try_from(super::read_bound(1)?)?);
    malformed.push(receipts);
    Ok(malformed)
}

#[test]
fn actual_counters_overflow_even_when_the_derived_bound_saturates() -> TestResult {
    let mut index = support::index();
    index.limits.record_bytes = u64::MAX;
    index.field_origins = Some(super::maximum_receipts());
    let budget = Budget::for_index(&index, Encoding::Pretty)?;
    assert_eq!(budget.bound, u64::MAX);
    assert!(budget.admit(u64::MAX, 1).is_err());
    let mut counter = Counter(u64::MAX);
    assert!(counter.write_all(b"x").is_err());
    assert_eq!(
        counter.0,
        u64::MAX,
        "failed actual addition never wraps or saturates"
    );
    Ok(())
}

#[test]
fn actual_hash_and_pretty_index_writer_use_the_same_control_budget() -> TestResult {
    support::run(|root, checkpoint| {
        let mut index = support::index();
        exact_old_limit(&mut index, Encoding::Pretty)?;
        index.field_origins = Some(super::maximum_receipts());
        let generation = index.generation.clone();
        let base = index.artifact_base.clone();
        let fingerprint = hashing::generation(&mut index, &checkpoint)?;
        assert_eq!(fingerprint.len(), 64);
        assert_eq!(index.generation, generation);
        assert_eq!(index.artifact_base, base);
        let output = root.join("written");
        fs::create_dir_all(&output)?;
        let (destination, previous) =
            IndexDestination::bind(&output, Limits::default(), &checkpoint)?;
        assert!(previous.is_none());
        let path = output.join("media-index.json");
        publish(&index, &path, &destination, &checkpoint)?;
        let committed = fs::read(&path)?;
        assert_eq!(committed, bytes(&index, Encoding::Pretty)?);
        index.source.path.push('x');
        assert!(publish(&index, &path, &destination, &checkpoint).is_err());
        assert_eq!(
            fs::read(&path)?,
            committed,
            "failed budget preserves the committed root index"
        );
        index.limits.record_bytes = 1;
        assert!(hashing::generation(&mut index, &checkpoint).is_err());
        assert_eq!(index.generation, generation);
        assert_eq!(index.artifact_base, base);
        Ok(())
    })
}

#[test]
fn verified_historical_payload_files_do_not_make_none_a_current_reuse_candidate() -> TestResult {
    support::run(|root, checkpoint| {
        let output = root.join("legacy");
        let mut index = support::index();
        index.generation = hashing::generation(&mut index, &checkpoint)?;
        index.artifact_base = format!("generations/{}", index.generation);
        support::legacy_files(&output, &index)?;
        assert!(
            generation_validation::existing(&output, Limits::default(), &checkpoint)?.is_none()
        );
        Ok(())
    })
}

#[test]
fn a_closed_some_group_without_its_actual_receipt_files_cannot_be_reused() -> TestResult {
    support::run(|root, checkpoint| {
        let output = root.join("unproved");
        let mut index = support::index();
        index.field_origins = Some(super::maximum_receipts());
        index.generation = hashing::generation(&mut index, &checkpoint)?;
        index.artifact_base = format!("generations/{}", index.generation);
        support::legacy_files(&output, &index)?;
        let result = generation_validation::existing(&output, Limits::default(), &checkpoint);
        assert!(
            matches!(result,
            Err(MediaError::Destination(message)) if message == "origin artifact is missing"),
            "current reuse must reach the actual receipt-file verification"
        );
        Ok(())
    })
}
fn publish(
    index: &Index,
    path: &Path,
    destination: &IndexDestination,
    checkpoint: &Checkpoint<'_>,
) -> Result<(), MediaError> {
    index_file::write(
        index,
        path,
        Publication::RootIndex(destination),
        checkpoint,
        &|_| Ok(()),
    )
}
#[test]
fn actual_hash_retains_the_exact_compact_base_limit_and_restores_both_fields() -> TestResult {
    support::run(|_root, checkpoint| {
        let mut index = support::index();
        let generation = std::mem::take(&mut index.generation);
        let base = std::mem::take(&mut index.artifact_base);
        exact_old_limit(&mut index, Encoding::Compact)?;
        index.generation = generation.clone();
        index.artifact_base = base.clone();
        index.field_origins = Some(super::maximum_receipts());
        assert_eq!(hashing::generation(&mut index, &checkpoint)?.len(), 64);
        assert_eq!(index.generation, generation);
        assert_eq!(index.artifact_base, base);
        index.source.path.push('x');
        assert!(matches!(hashing::generation(&mut index, &checkpoint),
            Err(MediaError::Limit { budget: "index_bytes", limit }) if limit == index.limits.record_bytes));
        assert_eq!(index.generation, generation);
        assert_eq!(index.artifact_base, base);
        Ok(())
    })
}
