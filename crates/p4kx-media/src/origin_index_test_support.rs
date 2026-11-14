// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken};
use p4kx_testkit::ArchiveBuilder;
use sha2::{Digest as _, Sha256};

use crate::checkpoint::Checkpoint;
use crate::{
    Algorithm, Artifact, ByteScope, Digest, Index, Limits, MediaFormat, MediaVersion, RootSource,
    Tally,
};

pub(super) type TestResult = Result<(), Box<dyn Error>>;

pub(super) fn index() -> Index {
    Index {
        format: MediaFormat::P4kxMedia,
        format_version: MediaVersion::V1,
        complete: true,
        enumeration_complete: true,
        source: RootSource {
            path: "synthetic.p4k".into(),
            digest: Digest {
                algorithm: Algorithm::Sha256,
                scope: ByteScope::RootArchiveFile,
                hex: "0".repeat(64),
            },
            size: 0,
            build: None,
            build_origin: None,
            build_present: true,
            installation_flags: None,
            directory_issue_count: 0,
            directory_issues: Vec::new(),
            directory_issue_samples_complete: true,
            enumeration_complete: true,
        },
        generation: "0".repeat(64),
        artifact_base: format!("generations/{}", "0".repeat(64)),
        limits: Limits::default(),
        source_entries: Tally::default(),
        kinds: Vec::new(),
        source_manifest: artifact("sources.jsonl"),
        artifact_manifest: artifact("artifacts.jsonl"),
        field_origins: None,
    }
}

pub(super) fn artifact(path: &str) -> Artifact {
    Artifact {
        path: path.into(),
        size: 0,
        digest: Digest {
            algorithm: Algorithm::Sha256,
            scope: ByteScope::ArtifactFile,
            hex: crate::hashing::hex(Sha256::digest(b"").into()),
        },
    }
}

pub(super) fn run(operation: impl FnOnce(&Path, Checkpoint<'_>) -> TestResult) -> TestResult {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("Data.p4k");
    ArchiveBuilder::new().write_to(&source)?;
    let archive = Archive::open(&source)?;
    let cancel = CancelToken::default();
    let watch = ArchiveWatch::new(&archive);
    let protection = ArchiveWriteProtection::new(&source, cancel.flag())?;
    operation(
        directory.path(),
        Checkpoint {
            cancel: &cancel,
            watch: &watch,
            protection: &protection,
        },
    )
}

pub(super) fn legacy_files(root: &Path, index: &Index) -> TestResult {
    let generation = root.join(&index.artifact_base);
    fs::create_dir_all(&generation)?;
    fs::write(generation.join(&index.source_manifest.path), b"")?;
    fs::write(generation.join(&index.artifact_manifest.path), b"")?;
    fs::write(
        root.join("media-index.json"),
        serde_json::to_vec_pretty(index)?,
    )?;
    Ok(())
}
