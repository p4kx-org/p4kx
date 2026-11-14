// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_extraction::{Archive, Exclusion, Filter};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::DirectoryLookup;
use crate::directory_index::DirectoryIndex;
use crate::source_selection::SourceSelection;
use crate::{
    Algorithm, ByteScope, CandidateState, Digest, Identity, MediaKind, NotSelectedReason,
    ReferenceCandidate, ReferenceUnavailable,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn lossy_path_evidence_precedes_filter_and_unrequested_kind_classification() -> TestResult {
    for (pattern, requested) in [("*", true), ("*a.mtl", true), ("*", false)] {
        let candidate = candidate(true, pattern, requested)?;
        assert!(
            matches!(
                candidate.state,
                CandidateState::Unavailable {
                    reason: ReferenceUnavailable::UnsupportedLayout
                }
            ),
            "a lossy display coincidence never proves a selected or excluded target"
        );
        let Identity::Entry { entry, .. } = candidate.target.source else {
            return Err("physical candidate identity".into());
        };
        assert_eq!(
            entry.raw_path_hex.as_deref(),
            Some("446174612f626164ff2e6d746c"),
            "unsupported display matches preserve original path bytes"
        );
    }
    Ok(())
}

#[test]
fn exact_paths_keep_filter_and_unrequested_kind_meanings() -> TestResult {
    assert!(
        matches!(
            candidate(false, "*a.mtl", true)?.state,
            CandidateState::NotSelected {
                reason: NotSelectedReason::Filter
            }
        ),
        "a proved path may be classified by its actual filter"
    );
    assert!(
        matches!(
            candidate(false, "*", false)?.state,
            CandidateState::NotSelected {
                reason: NotSelectedReason::KindNotRequested
            }
        ),
        "a proved path may be outside the requested kind scope"
    );
    assert!(
        matches!(candidate(false, "*", true)?.state, CandidateState::Selected),
        "selected lossless paths keep their existing classification"
    );
    Ok(())
}

fn candidate(
    lossy: bool,
    pattern: &str,
    requested: bool,
) -> Result<ReferenceCandidate, Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("synthetic.p4k");
    let entry = TestEntry::stored("Data/bad.mtl", b"<Material/>");
    let entry = if lossy {
        entry.with_path_bytes(b"Data/bad\xff.mtl")
    } else {
        entry
    };
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    let archive = Archive::open(&path)?;
    let slots = Cell::new(0);
    let index = DirectoryIndex::new(&archive, &slots);
    let root = Digest {
        algorithm: Algorithm::Sha256,
        scope: ByteScope::RootArchiveFile,
        hex: "00".repeat(32),
    };
    let include = Filter::parse(pattern)?;
    let exclude = Exclusion::default();
    let selection = SourceSelection {
        include: &include,
        exclude: &exclude,
    };
    let work = Cell::new(0);
    let lookup = DirectoryLookup::new(&index, &root, &[], selection, &work, 1);
    Ok(lookup.candidate(
        archive.entries().first().ok_or("synthetic entry")?,
        Some(MediaKind::Material),
        requested,
    ))
}
