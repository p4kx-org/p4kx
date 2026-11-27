// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{BufReader, Write};
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogError, CatalogOptions, Compression};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

use super::{TestResult, scan, transfer};

fn fixture() -> Value {
    let repeated = ["a\"b", "z,[]{}"];
    json!({
        "schemaVersion": 3,
        "definitions": {
            "example": {
                "rows": [
                    {"sourceUsesRefs": repeated, "comment": "München \"sourceUsesRefs\": [\"decoy\"]"},
                    {"sourceUsesRefs": []}
                ]
            }
        },
        "items": [{"sourceUsesRefs": repeated}, {"sourceUsesRefs": ["distinct"]}],
        "provenance": {
            "sources": [
                {"id": "a", "field": {"sourceUsesRefs": repeated}},
                {"id": "b"}
            ],
            "uses": [{"id": "c"}, {"id": "d"}, {"id": "e"}]
        }
    })
}

#[test]
fn sections_and_reference_fingerprints_follow_utf8_and_chunk_boundaries() -> TestResult<()> {
    let document = fixture();
    let mut raw = serde_json::to_vec(&document)?;
    raw.push(b'\n');
    for capacity in [1, 2, 7, 8192] {
        let report = scan::read(BufReader::with_capacity(capacity, raw.as_slice()))?;
        assert_eq!(
            report.decoded_bytes,
            raw.len() as u64,
            "all decoded UTF-8 bytes"
        );
        assert_eq!(report.physical_sources, 2, "exact source rows");
        assert_eq!(report.source_uses, 3, "exact use rows");
        assert_eq!(report.framing_bytes, 6, "delimiters and trailing newline");
        for section in &report.sections {
            let container = match &section.group {
                Some(group) => document.get(group).ok_or("section group")?,
                None => &document,
            };
            let data = container.get(&section.name).ok_or("section value")?;
            let value_bytes = serde_json::to_vec(data)?.len() as u64;
            let key_bytes = serde_json::to_vec(&section.name)?.len() as u64;
            assert_eq!(
                section.value_bytes, value_bytes,
                "value bytes of {}",
                section.name
            );
            assert_eq!(
                section.member_bytes,
                key_bytes + 1 + value_bytes,
                "member bytes"
            );
        }
        let origin = &report.reference_sets;
        assert_eq!(
            origin.arrays, 5,
            "array count excludes text with a key-looking literal"
        );
        assert_eq!(origin.empty_arrays, 1, "empty contributor set");
        assert_eq!(origin.references, 7, "reference occurrences");
        let repeated_bytes = serde_json::to_vec(&["a\"b", "z,[]{}"])?.len() as u64;
        let distinct_bytes = serde_json::to_vec(&["distinct"])?.len() as u64;
        assert_eq!(
            origin.array_bytes,
            3 * repeated_bytes + distinct_bytes + 2,
            "source array bytes"
        );
        assert_eq!(
            origin.distinct_nonempty_sha256_fingerprints, 2,
            "two fingerprints"
        );
        assert_eq!(
            origin.repeated_nonempty_sha256_fingerprints, 1,
            "one recurring fingerprint"
        );
        assert_eq!(
            origin.nonempty_arrays_repeating_a_sha256_fingerprint, 2,
            "occurrences after first"
        );
    }
    Ok(())
}

#[test]
fn incomplete_and_trailing_catalog_streams_are_rejected() -> TestResult<()> {
    let mut raw = serde_json::to_vec(&fixture())?;
    assert!(
        scan::read(raw.as_slice()).is_err(),
        "the trailing newline is missing"
    );
    raw.push(b'\n');
    raw.extend(b"{}");
    assert!(
        scan::read(raw.as_slice()).is_err(),
        "a second object is not a catalog frame"
    );
    Ok(())
}

#[test]
fn rejected_normal_producer_generation_remains_the_primary_failure() {
    let result = transfer::<()>(|_| Err(CatalogError::Cancelled));
    assert!(
        matches!(result, Err(ref error) if matches!(error.downcast_ref::<CatalogError>(), Some(CatalogError::Cancelled))),
        "the reader reports an incomplete stream when the producer declines publication"
    );
}

#[test]
fn the_real_compressor_stream_reconciles_with_catalog_inventory() -> TestResult<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let dataforge = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("SCItemManufacturer").field("Code", TestType::String))
        .record(
            TestRecord::new(
                1,
                "SCItemManufacturer",
                "SCItemManufacturer.Sample",
                "libs/foundry/records/scitemmanufacturer/sample.xml",
            )
            .value("Code", TestValue::text("Sample")),
        )
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", dataforge))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let (summary, census) = transfer(|writer| {
        p4kx_export::write_catalog(
            &archive,
            CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
            Compression::ZSTD,
            || Ok(writer),
        )
    })?;
    census.reconcile(&summary)?;
    assert!(
        summary.is_complete(),
        "normal publication policy was enforced"
    );
    assert!(
        census.decoded.reference_sets.references > 0,
        "the catalog includes real source uses"
    );
    Ok(())
}

#[test]
fn damaged_zstd_input_does_not_produce_a_census() -> TestResult<()> {
    let mut raw = serde_json::to_vec(&fixture())?;
    raw.push(b'\n');
    let mut encoder = zstd::Encoder::new(Vec::new(), 1)?;
    encoder.include_checksum(true)?;
    encoder.write_all(&raw)?;
    let mut compressed = encoder.finish()?;
    let _removed = compressed.pop().ok_or("checksummed frame is empty")?;
    let result = transfer(|mut writer| {
        writer.write_all(&compressed).map_err(CatalogError::Write)?;
        Ok(((), writer))
    });
    assert!(
        result.is_err(),
        "a truncated checksummed frame has no receipt"
    );
    Ok(())
}
