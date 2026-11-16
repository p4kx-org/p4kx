// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::artifact::{Stream, StreamObserver};
use crate::checkpoint::Checkpoint;
use crate::{Limits, MediaError, MediaWriter, artifact_placement, retained_source};

#[test]
fn a_kind_stream_rechecks_the_physical_target_below_its_output_root()
-> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let archive = fixture(folder.path())?;
    let source = archive.path();
    let original = fs::read(source)?;
    let stage = folder.path().join("stage");
    fs::create_dir(&stage)?;
    let alias = stage.join("archive.jsonl");
    fs::hard_link(source, &alias)?;
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
    assert_eq!(fs::read(source)?, original);
    assert_eq!(fs::read(&alias)?, original);
    Ok(())
}

#[test]
fn observing_streams_cannot_stage_or_place_payload_files() -> Result<(), Box<dyn std::error::Error>>
{
    let folder = tempfile::tempdir()?;
    let archive = fixture(folder.path())?;
    let stage = folder.path().join("stage");
    fs::create_dir(&stage)?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let watch = ArchiveWatch::new(&archive);
    let observer = Discard;
    let mut writer = MediaWriter::observed(
        &stage,
        Vec::new(),
        Limits::default(),
        Checkpoint {
            cancel: &cancel,
            watch: &watch,
            protection: &protection,
        },
        false,
        &observer,
    )?;
    let entry = archive.entries().first().ok_or("synthetic source Entry")?;
    assert_file_placement_rejected(
        retained_source::retain(
            &archive,
            entry,
            retained_source::PayloadVerification::Ordinary,
            &mut writer,
        )
        .err()
        .as_ref(),
    );
    assert_file_placement_rejected(
        artifact_placement::temporary(&writer, &stage, "test artifact")
            .err()
            .as_ref(),
    );
    assert!(
        fs::read_dir(&stage)?.next().is_none(),
        "observation leaves no files"
    );
    Ok(())
}

fn fixture(root: &Path) -> Result<Archive, Box<dyn std::error::Error>> {
    let source = root.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("data.bin", b"synthetic"))
        .write_to(&source)?;
    Ok(Archive::open(&source)?)
}

fn assert_file_placement_rejected(error: Option<&MediaError>) {
    assert!(
        matches!(error, Some(MediaError::Contract(cause))
            if cause == "file placement requires a file-backed Media writer"),
        "unexpected error: {error:?}",
    );
}

struct Discard;

impl StreamObserver for Discard {
    fn write(&self, _relative: &str, _bytes: &[u8]) -> io::Result<()> {
        Ok(())
    }
}
