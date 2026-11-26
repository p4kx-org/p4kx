// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{
    ArchiveWriteProtection, ConflictPolicy, ConversionError, WriteProtectionError,
};

use super::super::plan::Job;
use super::super::{ExportOptions, ExportWatch, SoundFormat, WemExportSource};
use super::{ExportState, emit};

fn job() -> Job {
    Job {
        trigger: "Play".to_owned(),
        stem: "Play".to_owned(),
        wem_id: 1,
        source: WemExportSource::Loose { entry: 0 },
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
        None,
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
        None,
        &path,
        None,
        (&job(), options, &watch),
    )
    .expect("first");
    let companion = first.origin_receipt.as_ref().expect("written companion");
    let previous = fs::read(companion).expect("original companion");
    let second = emit(
        SoundFormat::Raw,
        None,
        &[7; 64],
        None,
        &path,
        None,
        (&job(), options, &watch),
    )
    .expect("second");

    assert_eq!((first.size, second.size), (64, 64));
    assert_eq!(
        (first.state, second.state),
        (ExportState::Written, ExportState::Kept),
        "identical artifact was kept"
    );
    assert!(second.origin_receipt.is_none(), "kept bytes add no proof");
    assert_eq!(
        fs::read(companion).expect("kept companion"),
        previous,
        "proof is unchanged"
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
        None,
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

#[test]
fn replace_preserves_a_foreign_companion_and_reports_the_new_audio() {
    let dir = tempfile::tempdir().expect("folder");
    let path = dir.path().join("Play.wem");
    let companion = dir.path().join("Play.wem.p4kx-origin.jsonl");
    fs::write(&path, b"old sound").expect("old artifact");
    fs::write(&companion, b"foreign companion").expect("foreign file");
    let watch = ExportWatch::default();
    let options = ExportOptions {
        format: SoundFormat::Raw,
        policy: ConflictPolicy::Overwrite,
    };
    let output = emit(
        SoundFormat::Raw,
        None,
        b"new sound",
        None,
        &path,
        None,
        (&job(), options, &watch),
    )
    .expect("replacement");

    assert_eq!(output.state, ExportState::Written, "artifact was replaced");
    assert!(output.origin_receipt.is_none(), "foreign file has no proof");
    assert!(
        output.receipt_failure.is_none(),
        "foreign companion is preserved by policy"
    );
    assert_eq!(
        fs::read(&path).expect("new audio"),
        b"new sound",
        "installed bytes"
    );
    assert_eq!(
        fs::read(&companion).expect("foreign companion"),
        b"foreign companion",
        "foreign bytes are preserved"
    );
}

#[test]
fn a_late_skip_reports_the_incumbent_and_leaves_its_companion() {
    let dir = tempfile::tempdir().expect("folder");
    let path = dir.path().join("Play.wem");
    let companion = dir.path().join("Play.wem.p4kx-origin.jsonl");
    let incumbent = b"incumbent artifact";
    fs::write(&path, incumbent).expect("incumbent");
    fs::write(&companion, b"companion").expect("existing sibling");
    let watch = ExportWatch::default();
    let options = ExportOptions {
        format: SoundFormat::Raw,
        policy: ConflictPolicy::Skip,
    };
    let output = emit(
        SoundFormat::Raw,
        None,
        b"new sound",
        None,
        &path,
        None,
        (&job(), options, &watch),
    )
    .expect("keep");

    assert_eq!(output.state, ExportState::Kept, "no installation");
    assert_eq!(
        output.size,
        incumbent.len() as u64,
        "size of incumbent artifact"
    );
    assert!(output.origin_receipt.is_none(), "no proof for kept bytes");
    assert_eq!(
        fs::read(&path).expect("kept audio"),
        incumbent,
        "incumbent stays"
    );
    assert_eq!(
        fs::read(&companion).expect("kept sibling"),
        b"companion",
        "proof stays"
    );
    assert_eq!(
        fs::read_dir(dir.path()).expect("folder").count(),
        2,
        "no stage or lock remains"
    );
}
