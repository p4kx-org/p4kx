// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, CancelToken, ConflictPolicy};
use p4kx_media::{ExportOptions, Limits, MediaError, MediaKind, ProviderRegistry};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::support::{self, TestResult};

#[test]
fn unavailable_explicit_kind_is_not_an_empty_success_even_with_no_entries() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new().write_to(&path)?;
    let archive = Archive::open(path)?;
    let output = folder.path().join("media");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        ..ExportOptions::default()
    };
    assert!(
        matches!(
            p4kx_media::export_with(
                &archive,
                &output,
                &options,
                &ProviderRegistry::try_new(Vec::new())?
            ),
            Err(MediaError::Unsupported {
                kind: MediaKind::AnimationDefinition,
                ..
            })
        ),
        "unregistered kind is a typed failure"
    );
    assert!(
        !output.exists(),
        "capability refusal occurs before output creation"
    );
    Ok(())
}

#[test]
fn root_entry_source_and_record_budgets_leave_the_previous_generation_current() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    p4kx_media::export(&archive, &output, &ExportOptions::default())?;
    let previous = fs::read(output.join("media-index.json"))?;
    let mut cases = Vec::new();
    for name in ["root", "entry", "source", "records", "output", "line"] {
        let mut limits = Limits::default();
        match name {
            "root" => limits.root_bytes = 1,
            "entry" => limits.entry_bytes = 1,
            "source" => limits.source_bytes = 1,
            "records" => limits.records = 1,
            "output" => limits.output_bytes = 1,
            _ => limits.record_bytes = 16,
        }
        cases.push((name, limits));
    }
    for (name, limits) in cases {
        let options = ExportOptions {
            conflict: ConflictPolicy::Overwrite,
            limits,
            ..ExportOptions::default()
        };
        assert!(
            p4kx_media::export(&archive, &output, &options).is_err(),
            "{name} budget must fail"
        );
        assert_eq!(
            fs::read(output.join("media-index.json"))?,
            previous,
            "{name} did not replace complete index"
        );
    }
    Ok(())
}

#[test]
fn cancellation_and_corrupt_input_do_not_publish_a_new_generation() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let token = CancelToken::new();
    token.cancel();
    let output = folder.path().join("cancelled");
    let options = ExportOptions {
        cancel: token,
        ..ExportOptions::default()
    };
    assert!(matches!(
        p4kx_media::export(&archive, &output, &options),
        Err(MediaError::Cancelled)
    ));
    assert!(
        !output.join("media-index.json").exists(),
        "cancelled generation has no commit point"
    );
    let path = folder.path().join("bad.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("bad", b"bad").with_crc32c(1))
        .write_to(&path)?;
    let bad = Archive::open(path)?;
    let output = folder.path().join("bad-output");
    assert!(
        p4kx_media::export(&bad, &output, &ExportOptions::default()).is_err(),
        "verified extraction must reject source corruption"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "corrupt source has no successful manifest"
    );
    Ok(())
}
