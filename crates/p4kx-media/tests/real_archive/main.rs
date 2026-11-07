// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Opt-in bounded preservation verification without game fixtures or keys in source control.

use std::fs;
use std::io::Read;

use p4kx_extraction::{Archive, ArchiveWatch, Checksum, Exclusion, Filter};
use p4kx_media::{Algorithm, ByteScope, ExportOptions, ExportOutcome, MediaRecord};
use p4kx_testkit::real_archive_path;

mod manifests;
mod privacy;
use sha2::{Digest as _, Sha256};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn selected_real_payloads_match_physical_entries_manifests_and_sidecar_hashes()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify())
}

fn verify() -> TestResult {
    let archive = Archive::open(real_archive_path()?)?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let candidate = archive
        .entries()
        .iter()
        .find(|entry| {
            !entry.is_encrypted()
                && entry.uncompressed_size() > 0
                && entry.uncompressed_size() <= 256 << 10
                && !entry.path().contains(['*', '?'])
                && !entry.path().to_ascii_lowercase().ends_with(".pak")
                && !entry.path().to_ascii_lowercase().ends_with(".socpak")
        })
        .ok_or("no deterministic small unencrypted root Entry")?;
    let filter = Filter::parse(candidate.path())?;
    let excluded = Exclusion::parse(["*.pak", "*.socpak"])?;
    let selected: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| filter.matches(entry.path()) && !excluded.matches(entry.path()))
        .collect();
    let size: u64 = selected.iter().map(|entry| entry.uncompressed_size()).sum();
    assert!(
        selected.len() <= 64 && size <= 8 << 20,
        "declared real-Archive selection exceeds 64 Entries or 8 MiB"
    );
    let mut options = ExportOptions {
        include: Some(filter),
        exclude: excluded,
        ..ExportOptions::default()
    };
    options.limits.entry_bytes = 1 << 20;
    options.limits.source_bytes = 8 << 20;
    let folder = tempfile::tempdir()?;
    let result = p4kx_media::export(&archive, folder.path(), &options)?;
    let ExportOutcome::Published { index, .. } = result else {
        return Err("expected complete bounded generation".into());
    };
    assert_index(&index, selected.len())?;
    assert_retained_payloads(&archive, folder.path(), &index, &selected)?;
    manifests::verify(&archive, folder.path(), &index, &selected)?;
    watch.check()?;
    folder.close()?;
    Ok(())
}

fn assert_index(index: &p4kx_media::Index, count: usize) -> TestResult {
    assert_eq!(
        index.source_entries.written, count as u64,
        "every selected Entry is retained"
    );
    assert_eq!(
        index.source.digest.algorithm,
        Algorithm::Sha256,
        "root fingerprint algorithm"
    );
    assert_eq!(
        index.source.digest.scope,
        ByteScope::RootArchiveFile,
        "root fingerprint byte scope"
    );
    let schema = p4kx_media::index_schema();
    jsonschema::validator_for(&schema)?
        .validate(&serde_json::to_value(index)?)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn assert_retained_payloads(
    archive: &Archive,
    output: &std::path::Path,
    index: &p4kx_media::Index,
    selected: &[&p4kx_extraction::Entry],
) -> TestResult {
    let stream = fs::read_to_string(output.join(&index.artifact_base).join("archive.jsonl"))?;
    let records: Vec<MediaRecord> = stream
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let sources: Vec<_> = records
        .iter()
        .filter(|record| record.source.raw.is_some())
        .collect();
    assert_eq!(
        sources.len(),
        selected.len(),
        "exact source count differs from metadata and root count"
    );
    for (record, entry) in sources.into_iter().zip(selected.iter().copied()) {
        let raw = record.source.raw.as_ref().ok_or("raw retention")?;
        let actual = fs::read(output.join(&index.artifact_base).join(&raw.path))?;
        let mut source = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut source)?;
        assert!(
            actual == source,
            "retained bytes differ from the selected physical Entry"
        );
        assert!(
            Checksum::Sha256(Sha256::digest(&actual).into()).hex() == raw.digest.hex,
            "retained payload hash differs from its artifact declaration"
        );
    }
    Ok(())
}
