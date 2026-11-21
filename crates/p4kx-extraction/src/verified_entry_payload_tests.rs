// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::Write;

use p4kx_archive::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};

use crate::origin_artifact::OriginDigestWriter;
use crate::{ConflictPolicy, ExtractOptions, extract_verified_entry_payload};

type TestResult = Result<(), Box<dyn Error>>;

const CONTENT: &[u8] = b"<Loadout/>";

#[test]
fn sealed_payload_identifies_one_original_entry_and_its_decoded_contents() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Shared.xml", CONTENT))
        .entry(TestEntry::deflated("Data/Shared.xml", CONTENT).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(test_key()))?;
    let foreign = Archive::open_with_key(&path, Some(test_key()))?;
    let [clear, encrypted] = archive.entries() else {
        return Err("the duplicate path must select two physical Entries".into());
    };
    let foreign_entry = foreign
        .entries()
        .get(1)
        .ok_or("the foreign Entry is missing")?;
    let target = directory.path().join("output");
    fs::create_dir(&target)?;
    let (report, proof) = extract_verified_entry_payload(
        &archive,
        encrypted,
        &target,
        "retained.xml",
        &ExtractOptions::default(),
    );
    assert_eq!(
        report.extracted(),
        1,
        "one decoded payload was written: {report:?}"
    );
    assert!(report.is_complete(), "the original extraction completed");
    let proof = proof.ok_or("the actual write has no verified payload proof")?;
    assert!(
        proof.is_for(&archive, encrypted),
        "the proof belongs to the exact selected encrypted Entry"
    );
    assert!(
        !proof.is_for(&archive, clear) && !proof.is_for(&foreign, foreign_entry),
        "equal decoded bytes and path cannot bind a different physical Entry"
    );
    let expected = fs::read(directory.path().join("output/retained.xml"))?;
    assert_eq!(
        expected, CONTENT,
        "the original decoded bytes are unchanged"
    );
    let mut measure = OriginDigestWriter::new(std::io::sink());
    measure.write_all(&expected)?;
    assert_eq!(
        *proof.digest(),
        measure.finish()?,
        "the proof covers the decoded bytes"
    );
    Ok(())
}

#[test]
fn checksum_failure_and_kept_file_cannot_seal_payload() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let target = directory.path().join("output");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Corrupt.xml", CONTENT).with_crc32c(1))
        .entry(TestEntry::stored("Data/Good.xml", CONTENT))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let [corrupt, good] = archive.entries() else {
        return Err("the two fixture Entries are missing".into());
    };
    fs::create_dir(&target)?;
    let (failure, proof) = extract_verified_entry_payload(
        &archive,
        corrupt,
        &target,
        "corrupt.xml",
        &ExtractOptions::default(),
    );
    assert_eq!(
        failure.checksum_mismatches().count(),
        1,
        "the actual checksum error is preserved: {failure:?}"
    );
    assert!(proof.is_none(), "a failed decoded read has no proof");
    assert!(
        !target.join("corrupt.xml").exists(),
        "no failed payload was installed"
    );
    fs::write(target.join("keep.xml"), b"previous")?;
    let (report, proof) = extract_verified_entry_payload(
        &archive,
        good,
        &target,
        "keep.xml",
        &ExtractOptions {
            policy: ConflictPolicy::Skip,
            ..ExtractOptions::default()
        },
    );
    assert_eq!(report.skipped().len(), 1, "an existing payload was kept");
    assert!(
        proof.is_none(),
        "a retained previous file has no new original read"
    );
    assert_eq!(
        fs::read(target.join("keep.xml"))?,
        b"previous",
        "kept bytes are untouched"
    );
    Ok(())
}
