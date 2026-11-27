// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use super::write_replacing_counted;

fn names(folder: &std::path::Path) -> Vec<String> {
    fs::read_dir(folder)
        .expect("read folder")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

#[test]
fn identical_data_is_dropped_under_if_changed_and_no_part_file_stays() {
    let dir = tempfile::tempdir().expect("folder");
    let out = dir.path().join("a.bin");
    let first = write_replacing_counted(&out, true, |writer| {
        writer
            .write_all(b"data")
            .map_err(|source| super::write_error(&out, source))
    })
    .expect("first");
    assert_eq!((first.len, first.changed), (4, true));
    let same = write_replacing_counted(&out, true, |writer| {
        writer
            .write_all(b"data")
            .map_err(|source| super::write_error(&out, source))
    })
    .expect("same");
    assert_eq!((same.len, same.changed), (4, false));
    let other = write_replacing_counted(&out, true, |writer| {
        writer
            .write_all(b"date")
            .map_err(|source| super::write_error(&out, source))
    })
    .expect("other");
    assert!(other.changed);
    assert_eq!(fs::read(&out).expect("read"), b"date");
    let mut files = names(dir.path());
    files.sort_unstable();
    assert_eq!(files, ["a.bin"], "no staged payload remains");
}

#[test]
fn overwrite_replaces_identical_data() {
    let dir = tempfile::tempdir().expect("folder");
    let out = dir.path().join("a.bin");
    for _ in 0..2 {
        let replaced = write_replacing_counted(&out, false, |writer| {
            writer
                .write_all(b"data")
                .map_err(|source| super::write_error(&out, source))
        })
        .expect("write");
        assert!(replaced.changed);
    }
}

#[test]
fn a_failed_write_leaves_nothing_behind() {
    let dir = tempfile::tempdir().expect("folder");
    let out = dir.path().join("a.bin");
    let failed = write_replacing_counted(&out, true, |writer| {
        writer
            .write_all(b"half")
            .map_err(|source| super::write_error(&out, source))?;
        Err(super::write_error(&out, std::io::Error::other("stop")))
    });
    assert!(failed.is_err());
    assert_eq!(fs::read_dir(dir.path()).expect("read").count(), 0);
}

#[test]
fn cancellation_removes_a_fresh_ordinary_temporary_and_preserves_the_target() {
    use std::io::Write;

    let dir = tempfile::tempdir().expect("folder");
    let source = dir.path().join("source.p4k");
    let out = dir.path().join("result.bin");
    fs::write(&source, b"source").expect("source");
    fs::write(&out, b"prior").expect("prior output");
    let cancel = crate::CancelToken::default();
    let protection = crate::ArchiveWriteProtection::new(&source, cancel.flag()).expect("context");
    let result =
        super::write_replacing_protected(&out, Some(&protection), Some(&cancel), |writer| {
            writer
                .write_all(b"unpublished")
                .map_err(|error| super::write_error(&out, error))?;
            cancel.cancel();
            Ok(())
        });
    assert!(matches!(result, Err(crate::ConversionError::Cancelled)));
    assert_eq!(fs::read(&out).expect("output"), b"prior");
    assert_eq!(fs::read(&source).expect("source"), b"source");
    assert_eq!(names(dir.path()).len(), 2);
}

#[test]
fn a_retargeted_temporary_is_retained_as_a_failure_during_cancellation() {
    let dir = tempfile::tempdir().expect("folder");
    let source = dir.path().join("source.p4k");
    let out = dir.path().join("result.bin");
    let temporary = out.with_file_name(format!("result.bin.p4kx-{}.part", std::process::id()));
    let retained = dir.path().join("original.part");
    fs::write(&source, b"source").expect("source");
    let cancel = crate::CancelToken::default();
    let protection = crate::ArchiveWriteProtection::new(&source, cancel.flag()).expect("context");
    let result = super::write_replacing_protected(&out, Some(&protection), Some(&cancel), |_| {
        fs::rename(&temporary, &retained).expect("retarget owned namespace");
        fs::hard_link(&source, &temporary).expect("source alias");
        cancel.cancel();
        Ok(())
    });
    assert!(matches!(
        result,
        Err(crate::ConversionError::CleanupFailed { .. })
    ));
    assert_eq!(fs::read(&source).expect("source"), b"source");
    assert_eq!(
        fs::read(&temporary).expect("unowned alias remains"),
        b"source"
    );
    assert!(retained.is_file());
    assert!(!out.exists());
}
