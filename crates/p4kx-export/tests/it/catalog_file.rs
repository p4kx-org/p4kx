// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The catalog file written next to its target and moved over it, through the public API.

use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::Path;

use p4kx_export::{
    CancelToken, CatalogError, CatalogFile, CatalogOptions, Compression, write_catalog_file_from,
    write_catalog_from,
};
use p4kx_extraction::DataForgeEntry;

use super::catalog_compression::{archive, options};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn a_cancel_while_writing_leaves_no_file_and_the_old_file_intact() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let out = dir.path().join("catalog.json.zst");
    fs::write(&out, "old")?;
    let cancel = CancelToken::new();
    let options = CatalogOptions {
        cancel: Some(&cancel),
        ..options()
    };

    let result = write_catalog_from(&archive, &entry, &options, Compression::ZSTD, || {
        let file = CatalogFile::create(&out)?;
        cancel.cancel();
        Ok(file)
    });

    assert!(matches!(result, Err(CatalogError::Cancelled)), "cancelled");
    drop(result);
    assert_eq!(fs::read_to_string(&out)?, "old", "the old file is intact");
    assert_eq!(
        fs::read_dir(dir.path())?.count(),
        2,
        "only Data.p4k and the file"
    );
    Ok(())
}

#[test]
fn a_committed_file_replaces_the_old_one_and_leaves_no_temporary_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let out = dir.path().join("catalog.json.zst");
    fs::write(&out, "old")?;

    let (_, file) = write_catalog_from(&archive, &entry, &options(), Compression::ZSTD, || {
        CatalogFile::create(&out)
    })?;
    assert_eq!(
        fs::read_to_string(&out)?,
        "old",
        "untouched before the commit"
    );
    file.commit()?;

    let mut decoded = Vec::new();
    zstd::Decoder::new(fs::File::open(&out)?)?.read_to_end(&mut decoded)?;
    assert!(
        decoded.starts_with(b"{\"schemaVersion\":3"),
        "the new catalog"
    );
    assert_eq!(
        fs::read_dir(dir.path())?.count(),
        2,
        "only Data.p4k and the file"
    );
    Ok(())
}

#[test]
fn a_dropped_file_and_a_path_without_a_name_leave_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("catalog.json");

    drop(CatalogFile::create(&out)?);

    assert_eq!(fs::read_dir(dir.path())?.count(), 0, "nothing left");
    assert!(
        CatalogFile::create(Path::new("..")).is_err(),
        "no file name"
    );
    Ok(())
}

#[test]
fn protected_catalog_cancellation_explicitly_removes_the_created_temporary() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let out = dir.path().join("catalog.json");
    fs::write(&out, b"prior")?;
    let cancel = CancelToken::new();
    let protection = p4kx_extraction::ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let options = CatalogOptions {
        cancel: Some(&cancel),
        ..options()
    };
    let result = write_catalog_file_from(&archive, &entry, &options, Compression::None, || {
        let file = CatalogFile::create_protected(&out, &protection, &cancel)?;
        cancel.cancel();
        Ok(file)
    });
    assert!(matches!(result, Err(CatalogError::Cancelled)));
    assert_eq!(fs::read(&out)?, b"prior");
    assert_eq!(fs::read_dir(dir.path())?.count(), 2);
    Ok(())
}

#[test]
fn a_changed_temporary_does_not_hide_cleanup_failure_as_catalog_cancellation() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let source_before = fs::read(archive.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let out = dir.path().join("catalog.json");
    let temporary = out.with_file_name(format!("catalog.json.p4kx-{}.part", std::process::id()));
    let retained = dir.path().join("original.part");
    let cancel = CancelToken::new();
    let protection = p4kx_extraction::ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let options = CatalogOptions {
        cancel: Some(&cancel),
        ..options()
    };
    let result = write_catalog_file_from(&archive, &entry, &options, Compression::None, || {
        let file = CatalogFile::create_protected(&out, &protection, &cancel)?;
        fs::rename(&temporary, &retained)?;
        fs::hard_link(archive.path(), &temporary)?;
        cancel.cancel();
        Ok(file)
    });
    let CatalogError::CleanupFailed { primary, .. } = result.expect_err("typed retained output")
    else {
        return Err("cleanup failure was hidden".into());
    };
    assert!(matches!(*primary, CatalogError::Cancelled));
    assert_eq!(fs::read(archive.path())?, source_before);
    assert!(retained.is_file());
    assert!(temporary.is_file());
    assert!(!out.exists());
    Ok(())
}

#[test]
fn a_cancelled_build_never_calls_the_file_factory() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let options = CatalogOptions {
        cancel: Some(&cancel),
        ..options()
    };
    let mut called = false;
    let result = write_catalog_file_from(&archive, &entry, &options, Compression::None, || {
        called = true;
        CatalogFile::create(&dir.path().join("catalog.json"))
    });
    assert!(matches!(result, Err(CatalogError::Cancelled)));
    assert!(!called);
    assert_eq!(fs::read_dir(dir.path())?.count(), 1);
    Ok(())
}
