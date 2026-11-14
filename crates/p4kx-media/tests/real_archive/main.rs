// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Opt-in bounded preservation verification without game fixtures or keys in source control.

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, Checksum, Entry, Exclusion, Filter};
use p4kx_media::{Algorithm, ByteScope, ExportOptions, ExportOutcome, Index, MediaKind, Source};
use sha2::Sha256;

mod animation_accounting;
mod animation_event_accounting;
mod animation_graph_accounting;
mod animation_image_accounting;
mod animation_image_export;
mod animation_samples;
mod archive_replay;
mod audio_accounting;
mod layout_footer;
mod layout_inventory;
mod layout_records;
mod manifests;
mod material_accounting;
mod model_collision_frames;
mod model_collision_replay;
mod model_lod_distance_replay;
mod model_material_witness;
mod preservation_provider;
mod privacy;
mod shader_accounting;
mod shader_artifacts;
mod shader_expected;
mod shader_inventory;
mod shader_record_accounting;
mod shader_records;
mod texture_accounting;
mod texture_transfer;

use preservation_provider::PreservationProvider;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const MAX_SELECTED_ENTRIES: usize = 64;
const MAX_SOURCE_BYTES: u64 = 8 << 20;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn selected_real_payloads_match_physical_entries_manifests_and_sidecar_hashes()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify())
}

fn verify() -> TestResult {
    let archive = Archive::open(p4kx_testkit::real_archive_path()?)?;
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
    verify_archive(&archive, Filter::parse(candidate.path())?)?;
    watch.check()?;
    Ok(())
}

fn options(filter: Filter) -> TestResult<ExportOptions> {
    let mut options = ExportOptions {
        kinds: vec![MediaKind::Archive],
        include: Some(filter),
        exclude: Exclusion::parse(["*.pak", "*.socpak"])?,
        ..ExportOptions::default()
    };
    options.limits.entry_bytes = 1 << 20;
    options.limits.source_bytes = MAX_SOURCE_BYTES;
    options.limits.output_bytes = manifests::MAX_ARTIFACT_BYTES;
    Ok(options)
}

fn verify_archive(archive: &Archive, filter: Filter) -> TestResult {
    let options = options(filter)?;
    let include = options.include.as_ref().ok_or("selection filter")?;
    let selected: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| include.matches(entry.path()) && !options.exclude.matches(entry.path()))
        .take(MAX_SELECTED_ENTRIES + 1)
        .collect();
    let size = selected
        .iter()
        .try_fold(0_u64, |total, entry| {
            total.checked_add(entry.uncompressed_size())
        })
        .ok_or("declared selection size overflow")?;
    assert!(
        selected.len() <= MAX_SELECTED_ENTRIES && size <= MAX_SOURCE_BYTES,
        "declared real-Archive selection exceeds 64 Entries or 8 MiB"
    );
    let providers = p4kx_media::ProviderRegistry::try_new(vec![Box::new(PreservationProvider)])?;
    let folder = tempfile::tempdir()?;
    let result = p4kx_media::export_with(archive, folder.path(), &options, &providers)?;
    let ExportOutcome::Published { index, .. } = result else {
        return Err("expected complete bounded generation".into());
    };
    assert_index(&index, archive.entries().len(), selected.len())?;
    manifests::verify(archive, folder.path(), &index, &selected)?;
    assert_retained_payloads(archive, folder.path(), &index, &selected)?;
    folder.close()?;
    Ok(())
}

fn assert_index(index: &Index, physical: usize, selected: usize) -> TestResult {
    let physical = u64::try_from(physical)?;
    let selected = u64::try_from(selected)?;
    assert_eq!(
        index.source_entries.enumerated, physical,
        "physical inventory"
    );
    assert_eq!(
        index.source_entries.written, selected,
        "retained physical Entries"
    );
    assert_eq!(
        index.source_entries.skipped,
        physical - selected,
        "unselected Entries"
    );
    assert_eq!(
        index.kinds.len(),
        1,
        "only the preservation verification provider"
    );
    let kind = index.kinds.first().ok_or("verification provider")?;
    assert_eq!(
        kind.provider.id, "verification.selected-raw",
        "the explicit preservation provider owns this probe"
    );
    assert_eq!(
        kind.counts.source_entries.enumerated, selected,
        "provider callbacks count selected physical sources"
    );
    assert_eq!(
        kind.counts.source_entries.skipped, selected,
        "the verification provider adds no semantic projection"
    );
    assert_eq!(
        kind.counts.records.enumerated, 0,
        "no typed metadata projection"
    );
    assert_eq!(
        kind.counts.converted_outputs.enumerated, 0,
        "raw is not converted output"
    );
    assert_eq!(kind.artifact.size, 0, "no hidden semantic records");
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
    output: &Path,
    index: &Index,
    selected: &[&Entry],
) -> TestResult {
    use std::io::Read;

    use sha2::Digest as _;

    let base = output.join(&index.artifact_base);
    let stream = fs::read_to_string(base.join(&index.source_manifest.path))?;
    let sources: Vec<Source> = stream
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    assert_eq!(sources.len(), selected.len(), "exact retained source count");
    for (source, entry) in sources.iter().zip(selected.iter().copied()) {
        let raw = source.raw.as_ref().ok_or("raw retention")?;
        let actual = fs::read(base.join(&raw.path))?;
        let mut original = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut original)?;
        assert!(
            actual == original,
            "retained bytes differ from the physical Entry"
        );
        assert!(
            Checksum::Sha256(Sha256::digest(&actual).into()).hex() == raw.digest.hex,
            "retained hash differs from its artifact declaration"
        );
    }
    Ok(())
}
