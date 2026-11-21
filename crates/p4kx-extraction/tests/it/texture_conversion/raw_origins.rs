// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{
    ConflictPolicy, Exclusion, ExtractOptions, ExtractionReport, FolderSource, TextureConversion,
    convert_folder, extract,
};

use super::fixture::{Fixture, rock, run_with, texture_entries};

type TestResult = Result<(), Box<dyn Error>>;

const COLOR: &str = "Data/Textures/rock_diff.dds";
const RECEIPT: &str = "Data/Textures/rock_diff.dds.p4kx-origin.jsonl";
const MERGE_ONLY: TextureConversion = TextureConversion {
    merge: true,
    png: None,
    keep_raw: false,
};

#[test]
fn raw_archive_replacement_removes_the_owned_dds_receipt() -> TestResult {
    let target = published()?;

    let replacement = copy(&target, &target.target, false, false);
    assert!(replacement.is_complete(), "{replacement:?}");
    assert_eq!(
        target.read(COLOR)?,
        rock().header_bytes(),
        "raw DDS replaced merged bytes"
    );
    assert!(
        !target.target.join(RECEIPT).exists(),
        "obsolete owned proof was retired"
    );
    Ok(())
}

#[test]
fn successful_raw_fallback_removes_prior_merged_dds_receipt() -> TestResult {
    let target = published()?;
    let mut entries = texture_entries(&rock());
    entries.pop().ok_or("fixture has a required split mip")?;
    let broken = Fixture::new(entries)?;

    let replacement = copy(&broken, &target.target, true, false);
    assert_eq!(
        replacement.unconverted().len(),
        1,
        "the split merge cannot finish"
    );
    assert!(
        replacement.merged_texture_artifacts().is_empty(),
        "no new receipt was published"
    );
    assert_eq!(
        target.read(COLOR)?,
        rock().header_bytes(),
        "raw fallback really replaced the bytes"
    );
    assert!(
        !target.target.join(RECEIPT).exists(),
        "the old proof cannot follow raw bytes"
    );
    Ok(())
}

#[test]
fn a_failed_verified_dds_replacement_preserves_the_old_pair() -> TestResult {
    let target = published()?;
    let prior = target.read(RECEIPT)?;
    let mut entries = texture_entries(&rock());
    let unverified = entries.remove(0).with_crc32c(1);
    let broken = Fixture::new(vec![unverified])?;

    let replacement = copy(&broken, &target.target, true, true);
    assert!(
        !replacement.is_complete(),
        "failed verification cannot produce replacement"
    );
    assert_eq!(
        target.read(COLOR)?,
        rock().merged_bytes(),
        "the previous DDS is retained"
    );
    assert_eq!(
        target.read(RECEIPT)?,
        prior,
        "previous binding is byte-identical"
    );
    Ok(())
}

#[test]
fn a_foreign_sibling_is_untouched_when_writing_raw_dds() -> TestResult {
    let target = Fixture::new(texture_entries(&rock()))?;
    target.write(COLOR, b"old destination")?;
    target.write(RECEIPT, b"foreign sibling")?;

    let report = copy(&target, &target.target, false, false);
    assert!(report.is_complete(), "{report:?}");
    assert_eq!(
        target.read(COLOR)?,
        rock().header_bytes(),
        "raw bytes are installed"
    );
    assert_eq!(
        target.read(RECEIPT)?,
        b"foreign sibling",
        "foreign content is never retired"
    );
    Ok(())
}

#[test]
fn a_loose_dds_overwrite_retires_the_receipt_even_when_its_bytes_match() -> TestResult {
    let target = published()?;
    let prior_bytes = target.read(COLOR)?;
    let prior_receipt = target.read(RECEIPT)?;
    let input = tempfile::tempdir()?;
    fs::create_dir_all(input.path().join("Data/Textures"))?;
    fs::write(input.path().join(COLOR), &prior_bytes)?;
    let source = FolderSource::scan(input.path(), None)?;
    let selection = source.select(None, &Exclusion::default());
    let convert = |policy| {
        convert_folder(
            &source,
            &selection,
            true,
            &target.target,
            &ExtractOptions {
                policy,
                textures: TextureConversion {
                    merge: false,
                    ..MERGE_ONLY
                },
                ..ExtractOptions::default()
            },
        )
    };
    for policy in [ConflictPolicy::Skip, ConflictPolicy::IfChanged] {
        let report = convert(policy)?;
        assert_eq!(
            report.skipped().len(),
            1,
            "the existing DDS was kept: {report:?}"
        );
        assert_eq!(
            target.read(RECEIPT)?,
            prior_receipt,
            "keeping the DDS retains its proof"
        );
    }

    let report = convert(ConflictPolicy::Overwrite)?;
    assert!(report.is_complete(), "{report:?}");
    assert_eq!(report.extracted(), 1, "the new source replaced the DDS");
    assert_eq!(
        target.read(COLOR)?,
        prior_bytes,
        "the replacement bytes are identical"
    );
    assert!(
        !target.target.join(RECEIPT).exists(),
        "the new source has no Archive proof"
    );
    Ok(())
}

fn published() -> Result<Fixture, Box<dyn Error>> {
    let fixture = Fixture::new(texture_entries(&rock()))?;
    let report = run_with(&fixture, ConflictPolicy::Overwrite, MERGE_ONLY);
    if !report.is_complete() || report.merged_texture_artifacts().len() != 1 {
        return Err(format!("expected one merged texture with a prior receipt: {report:?}").into());
    }
    Ok(fixture)
}

fn copy(fixture: &Fixture, target: &Path, convert: bool, verify: bool) -> ExtractionReport {
    let entries = fixture.archive.entries().iter().collect::<Vec<_>>();
    extract(
        &fixture.archive,
        &entries,
        target,
        &ExtractOptions {
            policy: ConflictPolicy::Overwrite,
            textures: MERGE_ONLY,
            convert,
            verify,
            ..ExtractOptions::default()
        },
    )
}
