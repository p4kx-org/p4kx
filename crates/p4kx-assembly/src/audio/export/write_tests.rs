// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{
    ArchiveWriteProtection, ConflictPolicy, ConversionError, WriteProtectionError,
};

use super::super::plan::{Job, Origin};
use super::super::{ExportOptions, ExportWatch, SoundFormat};
use super::{ExportState, emit};

fn job() -> Job {
    Job {
        trigger: "Play".to_owned(),
        stem: "Play".to_owned(),
        wem_id: 1,
        origin: Origin::Loose(0),
    }
}

#[test]
fn a_write_after_cancel_fails_and_leaves_no_file_behind() {
    let dir = tempfile::tempdir().expect("folder");
    let watch = ExportWatch::default();
    watch.cancel();
    let options = ExportOptions {
        format: SoundFormat::Raw,
        policy: ConflictPolicy::Overwrite,
    };

    let emitted = emit(
        SoundFormat::Raw,
        None,
        &[7; 64],
        &dir.path().join("Play.wem"),
        None,
        (&job(), options, &watch),
    );

    assert!(emitted.is_err(), "the write stops");
    assert_eq!(fs::read_dir(dir.path()).expect("read").count(), 0);
}

#[test]
fn an_uncancelled_write_reports_the_bytes_it_wrote() {
    let dir = tempfile::tempdir().expect("folder");
    let options = ExportOptions {
        format: SoundFormat::Raw,
        policy: ConflictPolicy::IfChanged,
    };
    let path = dir.path().join("Play.wem");
    let watch = ExportWatch::default();

    let first = emit(
        SoundFormat::Raw,
        None,
        &[7; 64],
        &path,
        None,
        (&job(), options, &watch),
    )
    .expect("first");
    let second = emit(
        SoundFormat::Raw,
        None,
        &[7; 64],
        &path,
        None,
        (&job(), options, &watch),
    )
    .expect("second");

    assert_eq!((first.size, second.size), (64, 64));
    assert_eq!(
        (first.state, second.state),
        (ExportState::Written, ExportState::Kept)
    );
}

#[test]
fn source_bound_audio_output_refuses_the_original_source_file() {
    let dir = tempfile::tempdir().expect("folder");
    let source = dir.path().join("source.p4k");
    fs::write(&source, b"source").expect("source");
    let watch = ExportWatch::default();
    let protection = ArchiveWriteProtection::new(&source, watch.token.flag()).expect("context");
    let result = emit(
        SoundFormat::Raw,
        None,
        b"output",
        &source,
        Some(&protection),
        (
            &job(),
            ExportOptions {
                format: SoundFormat::Raw,
                policy: ConflictPolicy::Overwrite,
            },
            &watch,
        ),
    );
    assert!(matches!(
        result,
        Err(ConversionError::Installation(
            WriteProtectionError::SourceOverwrite { .. }
        ))
    ));
    assert_eq!(fs::read(&source).expect("source"), b"source");
    assert_eq!(fs::read_dir(dir.path()).expect("directory").count(), 1);
}
