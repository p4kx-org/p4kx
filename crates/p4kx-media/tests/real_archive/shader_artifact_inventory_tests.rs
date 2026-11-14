// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use p4kx_extraction::{Archive, ArchiveWatch, CancelToken, Checksum};
use p4kx_media::{Algorithm, Artifact, ByteScope, ExportOptions, ExportOutcome, Index, MediaKind};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use sha2::Sha256;

use super::super::shader_expected::ExpectedSource;
use super::super::{TestResult, shader_inventory, shader_record_accounting};
use super::{Retained, verify};

const FIRST: &[u8] = b"Property { Name = %A\n }\n";
const SECOND: &[u8] = b"Property { Name = %B\n }\n";

struct Generation {
    _directory: tempfile::TempDir,
    output: PathBuf,
    index: Index,
    sources: Vec<ExpectedSource>,
    enumerated: u64,
    watch: ArchiveWatch,
}

impl Generation {
    fn new(second: &[u8]) -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let archive_path = directory.path().join("Data.p4k");
        let nested = ArchiveBuilder::new()
            .entry(TestEntry::stored("Data/first.ext", FIRST))
            .entry(TestEntry::stored("Data/second.ext", second))
            .build();
        ArchiveBuilder::new()
            .entry(TestEntry::stored("Data/fixture.pak", nested))
            .write_to(&archive_path)?;
        let archive = Archive::open(&archive_path)?;
        let watch = ArchiveWatch::new(&archive);
        let shader_inventory::Selection {
            include,
            exclude,
            sources,
            enumerated,
            ..
        } = shader_inventory::discover(&archive, &watch, &CancelToken::new())?;
        assert_eq!(
            sources.len(),
            2,
            "the original discovery reads both physical sources"
        );
        let options = ExportOptions {
            kinds: vec![MediaKind::Shader],
            include: Some(include),
            exclude,
            ..ExportOptions::default()
        };
        let output = directory.path().join("media");
        let ExportOutcome::Published { index, .. } =
            p4kx_media::export(&archive, &output, &options)?
        else {
            return Err("the synthetic shader export is not complete".into());
        };
        Ok(Self {
            _directory: directory,
            output,
            index: *index,
            sources,
            enumerated,
            watch,
        })
    }

    fn verify_all(&self) -> TestResult<Vec<Retained>> {
        let retained = verify(&self.output, &self.index, &self.sources, &self.watch)?;
        let counts = shader_record_accounting::verify(
            &self.output,
            &self.index,
            &self.sources,
            &retained,
            self.enumerated,
            &self.watch,
        )?;
        assert_eq!(
            counts.definitions, 2,
            "both physical sources emit their own definition"
        );
        assert_eq!(
            counts.properties, 2,
            "both original Properties remain independently addressed"
        );
        assert_eq!(
            (counts.failed, counts.partial),
            (0, 0),
            "the positive export has no projection loss"
        );
        Ok(retained)
    }

    fn rewrite_artifacts(
        &mut self,
        mutate: impl FnOnce(&mut Vec<Artifact>) -> TestResult,
    ) -> TestResult {
        use sha2::Digest as _;

        let path = self
            .output
            .join(&self.index.artifact_base)
            .join(&self.index.artifact_manifest.path);
        let contents = std::fs::read_to_string(&path)?;
        let mut inventory: Vec<Artifact> = contents
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;
        assert_eq!(
            inventory.len(),
            2,
            "only two physical shader raw-retention receipts are present"
        );
        mutate(&mut inventory)?;
        let mut bytes = Vec::new();
        for artifact in &inventory {
            serde_json::to_writer(&mut bytes, artifact)?;
            bytes.push(b'\n');
        }
        std::fs::write(path, &bytes)?;
        let receipt = &mut self.index.artifact_manifest;
        assert_eq!(
            receipt.digest.algorithm,
            Algorithm::Sha256,
            "the actual manifest uses SHA-256"
        );
        assert_eq!(
            receipt.digest.scope,
            ByteScope::ArtifactFile,
            "the mutation rehashes the manifest file itself"
        );
        receipt.size = u64::try_from(bytes.len())?;
        receipt.digest.hex = Checksum::Sha256(Sha256::digest(&bytes).into()).hex();
        Ok(())
    }
}

#[test]
fn a_missing_raw_manifest_receipt_is_rejected_after_its_index_digest_is_updated() -> TestResult {
    let mut generation = Generation::new(SECOND)?;
    generation.verify_all()?;
    generation.rewrite_artifacts(|inventory| {
        let first = inventory
            .iter()
            .position(|artifact| artifact.path.starts_with("raw/"))
            .ok_or("actual raw artifact receipt")?;
        inventory.remove(first);
        Ok(())
    })?;
    assert!(
        verify(
            &generation.output,
            &generation.index,
            &generation.sources,
            &generation.watch
        )
        .is_err(),
        "one retained raw source cannot disappear from the artifact inventory"
    );
    Ok(())
}

#[test]
fn an_extra_raw_manifest_receipt_is_rejected_after_its_index_digest_is_updated() -> TestResult {
    let mut generation = Generation::new(SECOND)?;
    generation.verify_all()?;
    generation.rewrite_artifacts(|inventory| {
        let first = inventory
            .iter()
            .find(|artifact| artifact.path.starts_with("raw/"))
            .ok_or("actual raw artifact receipt")?
            .clone();
        inventory.push(first);
        Ok(())
    })?;
    assert!(
        verify(
            &generation.output,
            &generation.index,
            &generation.sources,
            &generation.watch
        )
        .is_err(),
        "another copy of a genuine receipt cannot fabricate a retention occurrence"
    );
    Ok(())
}

#[test]
fn identical_raw_bytes_from_two_physical_sources_preserve_receipt_multiplicity() -> TestResult {
    let generation = Generation::new(FIRST)?;
    let retained = generation.verify_all()?;
    let mut sources = retained.iter();
    let first = sources.next().ok_or("first retained physical source")?;
    let second = sources.next().ok_or("second retained physical source")?;
    assert!(
        sources.next().is_none(),
        "the synthetic family contains exactly two physical sources"
    );
    assert_ne!(
        first.source.identity, second.source.identity,
        "identical bytes do not merge physical Entries"
    );
    assert_eq!(
        first.source.raw, second.source.raw,
        "the content-addressed receipt may repeat for distinct physical source retentions"
    );
    Ok(())
}
