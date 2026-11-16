// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry, TestMtl, TestMtlMaterial, test_key};
use sha2::{Digest as _, Sha256};

use crate::{Artifact, ExportOutcome, Index, Limits, export, hashing};

use super::{Observed, Phase, TestResult, options, require, verify};

#[test]
fn material_observer_matches_the_file_based_export_with_nested_sources_and_dependencies()
-> TestResult {
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("parity.p4k");
    let output = directory.path().join("parity-output");
    fixture(&input)?;
    let archive = Archive::open_with_key(input, Some(test_key()))?;
    let mut phases = Vec::new();
    let observed = verify(&archive, Limits::default(), |phase| phases.push(phase))?;
    assert_eq!(
        phases,
        [
            Phase::HashRoot,
            Phase::CollectInputs,
            Phase::ReconcileStream,
            Phase::FinalizeStream,
            Phase::VerifyTotals,
        ],
        "the stream verifier reaches each phase in execution order"
    );
    assert_eq!(observed.counts.selected, 4, "root and nested sources");
    assert_eq!(observed.counts.nested, 2, "two nested sources");
    assert_eq!(observed.counts.complete, 3, "three valid libraries");
    assert_eq!(observed.counts.failed, 1, "one invalid nested library");
    let ExportOutcome::Incomplete { index, diagnostic } =
        export(&archive, &output, &options(Limits::default())?)?
    else {
        return Err("expected the small file-backed diagnostic generation".into());
    };
    let base = diagnostic
        .parent()
        .ok_or("diagnostic generation path")?
        .join(&index.artifact_base);
    receipts(&observed, &index)?;
    for line in fs::read_to_string(base.join(&index.artifact_manifest.path))?.lines() {
        let raw: Artifact = serde_json::from_str(line)?;
        let bytes = fs::read(base.join(&raw.path))?;
        assert_eq!(raw.size, u64::try_from(bytes.len())?, "actual raw bytes");
        assert_eq!(
            raw.digest.hex,
            hashing::hex(Sha256::digest(&bytes).into()),
            "actual raw SHA-256"
        );
    }
    drop(archive);
    directory.close()?;
    Ok(())
}

fn fixture(input: &Path) -> TestResult {
    let source = TestMtl::multi(&[TestMtlMaterial::new("retained")
        .attribute("Glow", "0")
        .attribute("Shader", "Illum")
        .layer_with(&[("Path", "source.mtl"), ("Submtl", "Authored")])
        .texture_with_mod("Diffuse", "missing.dds", &[("Amount", "3")])])
    .cryxml();
    let nested = TestContainer::new()
        .entry(TestEntry::stored("Data/source.mtl", source.as_slice()))
        .entry(TestEntry::stored("failed.mtl", b"<NotAMaterial/>"))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/source.mtl", source))
        .entry(TestEntry::stored(
            "unused.MTL",
            b"<Material Custom='root'/>",
        ))
        .entry(TestEntry::stored("inner.socpak", nested).encrypted())
        .write_to(input)?;
    Ok(())
}

fn receipts(observed: &Observed, index: &Index) -> TestResult {
    let result = &observed.finished;
    require(
        serde_json::to_value(&result.kinds)? == serde_json::to_value(&index.kinds)?,
        "material record bytes and tallies differ",
    )?;
    require(
        serde_json::to_value(observed.entries)? == serde_json::to_value(index.source_entries)?,
        "physical entry tallies differ",
    )?;
    let published = index
        .field_origins
        .as_ref()
        .ok_or("published origin receipts")?;
    require(
        serde_json::to_value(&result.origins)? == serde_json::to_value(published)?,
        "source, use, set or field-origin bytes differ",
    )?;
    for (streamed, file) in [
        (&result.sources, &index.source_manifest),
        (&result.artifacts, &index.artifact_manifest),
    ] {
        require(
            streamed.path == file.path
                && streamed.size == file.size
                && streamed.digest == file.digest,
            "source or raw manifest bytes differ",
        )?;
    }
    Ok(())
}
