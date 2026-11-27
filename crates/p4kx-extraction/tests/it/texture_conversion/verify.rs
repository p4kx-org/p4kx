// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{ConflictPolicy, ExtractOptions, TextureConversion, extract};

use super::fixture::{Fixture, HEADER, files, rock, texture_entries};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn a_part_with_a_wrong_checksum_fails_a_verified_texture_run() -> TestResult {
    let mut entries = texture_entries(&rock());
    let part = entries.remove(2).with_crc32c(1);
    entries.insert(2, part);
    let fixture = Fixture::new(entries)?;
    let all: Vec<_> = fixture.archive.entries().iter().collect();
    let options = ExtractOptions {
        policy: ConflictPolicy::Skip,
        convert: true,
        verify: true,
        ..ExtractOptions::default()
    };

    let report = extract(&fixture.archive, &all, &fixture.target, &options);

    let bad: Vec<_> = report
        .checksum_mismatches()
        .map(p4kx_extraction::EntryFailure::path)
        .collect();
    assert_eq!(
        bad,
        [format!("{HEADER}.2")],
        "the part is reported as a mismatch"
    );
    assert!(!report.is_complete(), "the wrong checksum fails the run");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.1",
            "Data/Textures/rock_diff.dds.1.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.dds.3",
            "Data/Textures/rock_diff.dds.3.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
        ],
        "the mismatching part is not written and the texture falls back to its raw Entries"
    );
    Ok(())
}

#[test]
fn a_header_with_a_flipped_data_byte_fails_a_verified_png_export() -> TestResult {
    let mut entries = texture_entries(&rock());
    let header = entries.remove(0);
    let mut payload = header.payload().to_vec();
    if let Some(last) = payload.last_mut() {
        *last ^= 0x01;
    }
    entries.insert(0, header.with_payload(payload));
    let fixture = Fixture::new(entries)?;
    let all: Vec<_> = fixture.archive.entries().iter().collect();
    let options = ExtractOptions {
        policy: ConflictPolicy::Skip,
        convert: true,
        textures: TextureConversion {
            merge: false,
            png: Some(0),
            keep_raw: false,
        },
        verify: true,
        ..ExtractOptions::default()
    };

    let report = extract(&fixture.archive, &all, &fixture.target, &options);

    let bad: Vec<_> = report
        .checksum_mismatches()
        .map(p4kx_extraction::EntryFailure::path)
        .collect();
    assert_eq!(bad, [HEADER], "the header is reported as a mismatch");
    assert!(!report.is_complete(), "the run fails");
    assert!(
        !files(&fixture.target)?.contains(&"Data/Textures/rock_diff.png".to_owned()),
        "no PNG is written from unverified header bytes"
    );
    Ok(())
}
