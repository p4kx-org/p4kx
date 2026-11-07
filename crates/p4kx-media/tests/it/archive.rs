// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, Checksum, Exclusion, Filter};
use p4kx_media::{Algorithm, ExportOptions, Identity};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};
use sha2::{Digest as _, Sha256};

use super::support::{self, TestResult};

#[test]
fn duplicate_physical_entries_retain_different_bytes_and_count_units() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    assert_eq!(
        (index.format, index.format_version),
        (
            p4kx_media::MediaFormat::P4kxMedia,
            p4kx_media::MediaVersion::V1
        )
    );
    let records = support::records(&output, &index, "archive.jsonl")?;
    assert_eq!(
        records.len(),
        4,
        "one root record plus three physical Entries"
    );
    let payloads = retained_payloads(&output, &index, &records)?;
    assert_eq!(
        payloads,
        [b"first".to_vec(), b"second".to_vec(), Vec::new()]
    );
    let positions: Vec<_> = records
        .iter()
        .filter_map(|record| match &record.source.identity {
            Identity::Entry { entry, .. } => Some(entry.entry_index),
            _ => None,
        })
        .collect();
    assert_eq!(
        positions,
        [0, 1, 2],
        "duplicate display paths have distinct physical indices"
    );
    let kind = index.kinds.first().ok_or("archive summary")?;
    assert_eq!(kind.counts.source_entries.written, 3);
    assert_eq!(kind.counts.records.written, 4);
    assert_eq!(
        kind.counts.converted_outputs.written, 0,
        "source retention is not conversion"
    );
    assert_eq!(
        index.source.digest.hex,
        Checksum::Sha256(Sha256::digest(fs::read(archive.path())?).into()).hex()
    );
    Ok(())
}

fn retained_payloads(
    output: &std::path::Path,
    index: &p4kx_media::Index,
    records: &[p4kx_media::MediaRecord],
) -> Result<Vec<Vec<u8>>, std::io::Error> {
    records
        .iter()
        .filter_map(|record| record.source.raw.as_ref())
        .map(|raw| {
            let bytes = fs::read(output.join(&index.artifact_base).join(&raw.path))?;
            assert_eq!(
                Checksum::Sha256(Sha256::digest(&bytes).into()).hex(),
                raw.digest.hex,
                "retained physical payload matches its declared artifact hash"
            );
            Ok(bytes)
        })
        .collect()
}

#[test]
fn nested_pak_and_socpak_keep_ordered_container_indices_and_crc_scope() -> TestResult {
    let folder = tempfile::tempdir()?;
    let inner = TestContainer::new()
        .entry(TestEntry::stored("same.bin", b"inside"))
        .build();
    let outer = TestContainer::new()
        .entry(TestEntry::stored("nested.pak", inner))
        .build();
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("world.socpak", outer))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let output = folder.path().join("media");
    let options = ExportOptions {
        include: Some(Filter::parse("same.bin")?),
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "archive.jsonl")?;
    let entry = rows
        .iter()
        .find(|row| row.source.raw.is_some())
        .ok_or("nested entry")?;
    assert_nested_identity(&entry.source)?;
    assert_eq!(
        entry
            .source
            .declared_integrity
            .as_ref()
            .ok_or("CRC")?
            .algorithm,
        Algorithm::Crc32
    );
    assert_eq!(
        entry.source.raw.as_ref().ok_or("raw")?.digest.algorithm,
        Algorithm::Sha256
    );
    assert_eq!(
        (
            index.source_entries.enumerated,
            index.source_entries.written,
            index.source_entries.skipped
        ),
        (3, 1, 2)
    );
    Ok(())
}

fn assert_nested_identity(source: &p4kx_media::Source) -> TestResult {
    let Identity::Entry { containers, entry } = &source.identity else {
        return Err("entry identity".into());
    };
    assert_eq!(
        containers
            .iter()
            .map(|container| container.path.as_str())
            .collect::<Vec<_>>(),
        ["world.socpak", "nested.pak"],
        "nested source identity keeps outer-to-inner container order"
    );
    assert_eq!(
        entry.entry_index, 0,
        "entry ordinal is local to the innermost container"
    );
    Ok(())
}

#[test]
fn exclusion_of_a_container_prevents_its_contents_from_being_opened() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("broken.socpak", b"not a container"))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let options = ExportOptions {
        exclude: Exclusion::parse(["*.socpak"])?,
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(
        &archive,
        &folder.path().join("media"),
        &options,
    )?)?;
    assert_eq!(
        (index.source_entries.written, index.source_entries.skipped),
        (0, 1)
    );
    Ok(())
}

#[test]
fn non_utf8_source_path_bytes_survive_the_display_path() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("lossy", b"payload").with_path_bytes(b"a\xff.bin"))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let output = folder.path().join("media");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    let rows = support::records(&output, &index, "archive.jsonl")?;
    let source = rows
        .iter()
        .find_map(|row| match &row.source.identity {
            Identity::Entry { entry, .. } => Some(entry),
            _ => None,
        })
        .ok_or("entry")?;
    assert_eq!(source.raw_path_hex.as_deref(), Some("61ff2e62696e"));
    Ok(())
}
