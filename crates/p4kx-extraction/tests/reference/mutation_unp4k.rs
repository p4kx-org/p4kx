// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Synthetic decoder-aware mutation checks of the unp4k byte allowance.

use std::io::Cursor;

use p4kx_extraction::{Archive, Compression, EffectiveEncoding};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};

use super::TestResult;
use super::bytes::{self, Verdict};
use super::tally::Tally;
use super::unp4k;
use super::unp4k_allow::{Context, PRODUCER_REVISION, SourceIdentity, TRAILING_ZEROS};

const PATH: &str = "Data\\sample.bin";
const ENCODINGS: [EffectiveEncoding; 4] = [
    EffectiveEncoding::Stored,
    EffectiveEncoding::PassThrough,
    EffectiveEncoding::Deflate,
    EffectiveEncoding::Zstandard,
];

const fn declared(encoding: EffectiveEncoding) -> Compression {
    match encoding {
        EffectiveEncoding::Stored => Compression::Store,
        EffectiveEncoding::Deflate => Compression::Deflate,
        _ => Compression::Zstd,
    }
}

const fn context(encoding: EffectiveEncoding, encrypted: bool) -> Context<'static> {
    Context {
        revision: PRODUCER_REVISION,
        patch_scope: None,
        source: SourceIdentity {
            path: PATH,
            compression: declared(encoding),
            encrypted,
        },
    }
}

fn encoded(encoding: EffectiveEncoding, data: &[u8]) -> TestEntry {
    match encoding {
        EffectiveEncoding::Stored => TestEntry::stored(PATH, data.to_vec()),
        EffectiveEncoding::Deflate => TestEntry::deflated(PATH, data.to_vec()),
        EffectiveEncoding::Zstandard => TestEntry::zstd(PATH, data.to_vec()),
        _ => TestEntry::zstd_without_magic(PATH, data.to_vec()),
    }
}

fn compare(
    encoding: EffectiveEncoding,
    encrypted: bool,
    data: &[u8],
    reference: &[u8],
    producer: Option<&Context<'_>>,
) -> Result<Tally, Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let entry = encoded(encoding, data);
    let entry = if encrypted { entry.encrypted() } else { entry };
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(test_key()))?;
    let entry = archive.entries().first().ok_or("Entry missing")?;
    let mut ours = archive.open_entry(entry)?;
    let verdict = bytes::compare_streams(&mut ours, Cursor::new(reference))?;
    assert_eq!(
        ours.effective_encoding(),
        encoding,
        "fixture selects its intended decoder"
    );
    let mut tally = Tally::default();
    unp4k::judge(
        &mut tally,
        SourceIdentity::of(entry),
        ours.effective_encoding(),
        producer,
        verdict,
    );
    Ok(tally)
}

fn assert_allowed(tally: &Tally) {
    assert_eq!(tally.allowed_items, 1, "one logical item is allowed");
    assert_eq!(
        tally.allowed.get(TRAILING_ZEROS),
        Some(&1),
        "producer-specific allowance is counted"
    );
    assert_eq!(tally.equal, 0, "a difference is not exact equality");
    assert_eq!(tally.different, 0, "no unexplained difference remains");
}

fn assert_different(tally: &Tally) {
    assert_eq!(tally.different, 1, "an unexplained difference is counted");
    assert_eq!(tally.allowed_items, 0, "no allowance is counted");
    assert_eq!(tally.equal, 0, "a difference is not exact equality");
}

#[test]
fn exact_bytes_stay_equal_for_every_effective_encoding_without_provenance() -> TestResult {
    let data = b"synthetic content\0\0\0";
    for encoding in ENCODINGS {
        for encrypted in [false, true] {
            let tally = compare(encoding, encrypted, data, data, None)?;
            assert_eq!(tally.equal, 1, "exact bytes remain equal");
            assert_eq!(tally.allowed_items, 0, "no allowance is counted");
            assert_eq!(tally.different, 0, "no unexplained difference remains");
        }
    }
    Ok(())
}

#[test]
fn only_encrypted_direct_bytes_allow_the_full_logical_zero_suffix() -> TestResult {
    let mut data = b"prefix".to_vec();
    data.resize(200_000, 0);
    for encoding in ENCODINGS {
        for encrypted in [false, true] {
            let producer = context(encoding, encrypted);
            let tally = compare(encoding, encrypted, &data, b"prefix", Some(&producer))?;
            if encrypted
                && matches!(
                    encoding,
                    EffectiveEncoding::Stored | EffectiveEncoding::PassThrough
                )
            {
                assert_allowed(&tally);
            } else {
                assert_different(&tally);
            }
        }
    }
    Ok(())
}

#[test]
fn removing_any_compressed_logical_zero_suffix_is_a_difference() -> TestResult {
    let mut data = b"prefix".to_vec();
    data.resize(100, 0);
    for encoding in [EffectiveEncoding::Deflate, EffectiveEncoding::Zstandard] {
        let producer = context(encoding, true);
        for removed in 1..100 {
            let reference = data.get(..data.len() - removed).ok_or("invalid suffix")?;
            assert_different(&compare(encoding, true, &data, reference, Some(&producer))?);
        }
    }
    Ok(())
}

#[test]
fn stored_retains_one_byte_even_when_that_byte_is_zero() -> TestResult {
    let encoding = EffectiveEncoding::Stored;
    let producer = context(encoding, true);
    for first in [0, 1] {
        let mut data = vec![0; 100];
        let byte = data.first_mut().ok_or("empty data")?;
        *byte = first;
        assert_allowed(&compare(encoding, true, &data, &[first], Some(&producer))?);
        for length in [0, 2, 3, 50] {
            let reference = data.get(..length).ok_or("invalid length")?;
            assert_different(&compare(encoding, true, &data, reference, Some(&producer))?);
        }
    }
    assert_different(&compare(
        encoding,
        true,
        &[1, 2, 0, 0],
        &[1, 2],
        Some(&producer),
    )?);
    Ok(())
}

#[test]
fn method_100_rewind_requires_a_full_four_byte_retained_prefix() -> TestResult {
    let encoding = EffectiveEncoding::PassThrough;
    let producer = context(encoding, true);
    for reference in [[0].as_slice(), &[1], &[1, 2, 3], &[1, 2, 3, 0]] {
        let mut original = reference.to_vec();
        original.extend([0; 32]);
        assert_different(&compare(
            encoding,
            true,
            &original,
            reference,
            Some(&producer),
        )?);
    }
    assert_allowed(&compare(
        encoding,
        true,
        &[1, 2, 3, 4, 0, 0],
        &[1, 2, 3, 4],
        Some(&producer),
    )?);
    Ok(())
}

#[test]
fn missing_or_mismatched_producer_and_source_evidence_cannot_allow_loss() -> TestResult {
    let encoding = EffectiveEncoding::PassThrough;
    let data = b"prefix\0\0\0";
    let valid = context(encoding, true);
    let mut contexts = [valid; 6];
    contexts[0].revision = "unknown";
    contexts[1].patch_scope = Some("local patch without reviewed trim semantics");
    contexts[2].source.path = "Data\\another.bin";
    contexts[3].source.encrypted = false;
    contexts[4].source.compression = Compression::Store;
    contexts[5].source.compression = Compression::Other(100);
    for producer in &contexts {
        assert_different(&compare(encoding, true, data, b"prefix", Some(producer))?);
    }
    assert_different(&compare(encoding, true, data, b"prefix", None)?);
    Ok(())
}

#[test]
fn unknown_or_inconsistent_effective_encoding_fails_closed() -> TestResult {
    let producer = context(EffectiveEncoding::Stored, true);
    let verdict = bytes::compare_streams(b"prefix\0\0".as_slice(), b"prefix".as_slice())?;
    for encoding in [
        EffectiveEncoding::Undetermined,
        EffectiveEncoding::Deflate,
        EffectiveEncoding::Zstandard,
        EffectiveEncoding::PassThrough,
    ] {
        let mut tally = Tally::default();
        unp4k::judge(
            &mut tally,
            producer.source,
            encoding,
            Some(&producer),
            verdict,
        );
        assert_different(&tally);
    }
    Ok(())
}

#[test]
fn added_changed_or_nonzero_lost_bytes_never_use_the_allowance() -> TestResult {
    let encoding = EffectiveEncoding::Stored;
    let producer = context(encoding, true);
    for reference in [
        b"prefix\0\0\0extra".as_slice(),
        b"prefiy\0\0".as_slice(),
        b"prefi".as_slice(),
    ] {
        assert_different(&compare(
            encoding,
            true,
            b"prefix\0\0",
            reference,
            Some(&producer),
        )?);
    }
    Ok(())
}

#[test]
fn a_verdict_without_an_actual_missing_tail_is_not_allowed() {
    let producer = context(EffectiveEncoding::Stored, true);
    for verdict in [
        Verdict::ReferenceShorter {
            zeros_only: true,
            extra: 1,
            reference_len: 1,
            last_reference_byte: None,
        },
        Verdict::ReferenceShorter {
            zeros_only: true,
            extra: 0,
            reference_len: 6,
            last_reference_byte: Some(b'x'),
        },
        Verdict::ReferenceShorter {
            zeros_only: false,
            extra: 1,
            reference_len: 6,
            last_reference_byte: Some(b'x'),
        },
    ] {
        let mut tally = Tally::default();
        unp4k::judge(
            &mut tally,
            producer.source,
            EffectiveEncoding::Stored,
            Some(&producer),
            verdict,
        );
        assert_different(&tally);
    }
}
