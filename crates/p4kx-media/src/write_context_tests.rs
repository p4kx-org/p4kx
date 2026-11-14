// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::MediaError;
use crate::artifact::Stream;
use crate::checkpoint::Checkpoint;

#[test]
fn a_kind_stream_rechecks_the_physical_target_below_its_output_root()
-> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("data.bin", b"synthetic"))
        .write_to(&source)?;
    let archive = Archive::open(&source)?;
    let original = fs::read(&source)?;
    let stage = folder.path().join("stage");
    fs::create_dir(&stage)?;
    let alias = stage.join("archive.jsonl");
    fs::hard_link(&source, &alias)?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let watch = ArchiveWatch::new(&archive);
    let checkpoint = Checkpoint {
        cancel: &cancel,
        watch: &watch,
        protection: &protection,
    };
    let error = Stream::create(&stage, "archive.jsonl".into(), &checkpoint)
        .err()
        .ok_or("source alias accepted as a kind artifact")?;
    assert!(matches!(error, MediaError::Protected(cause)
        if cause.installation().is_none() && cause.target() == Some(alias.as_path())));
    assert_eq!(fs::read(&source)?, original);
    assert_eq!(fs::read(&alias)?, original);
    Ok(())
}
