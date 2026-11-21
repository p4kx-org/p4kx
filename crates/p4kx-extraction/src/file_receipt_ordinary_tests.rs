// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use crate::output_file::{write_error, write_replacing_protected};
use crate::{
    OriginOutputAddress, SourceUses, write_bytes_replacing, write_origin_companion,
    write_replacing_counted, write_replacing_io_protected,
};

use super::{DirectoryReceipt, FileReceipt};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const PAYLOAD: &[u8] = b"same fixture bytes";
const FOREIGN: &[u8] = b"an unrelated user document";

fn publish_file(output: &FileReceipt<'_>) -> TestResult {
    let registry = SourceUses::new();
    let inventory = registry.freeze()?;
    let origin = registry.origin(&[], true);
    output.publish(
        |writer| writer.write_all(PAYLOAD),
        |writer, binding, ()| {
            write_origin_companion(
                writer,
                binding,
                &inventory,
                [(&OriginOutputAddress::OpaquePayload, &origin)],
            )
        },
    )?;
    Ok(())
}

fn publish_directory(dir: &Path, name: &str) -> TestResult {
    let output = DirectoryReceipt::new(dir, None)?;
    let ((), committed) = output.write(OsStr::new(name), |writer| {
        writer
            .write_all(PAYLOAD)
            .map_err(|source| write_error(&dir.join(name), source))
    })?;
    let registry = SourceUses::new();
    let inventory = registry.freeze()?;
    let fields = [(
        0,
        OriginOutputAddress::OpaquePayload,
        registry.origin(&[], true),
    )];
    assert!(
        output.publish([committed], &inventory, fields)?,
        "the directory receipt"
    );
    Ok(())
}

#[test]
fn ordinary_writers_retire_an_owned_receipt_even_when_bytes_are_identical() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("target.bin");
    let owner = FileReceipt::new(&path, None, None)?;
    for replace in ordinary_writers() {
        publish_file(&owner)?;
        assert!(owner.path().is_file(), "a prior complete companion");
        let lock = dir.path().join(super::lock::LOCK_NAME);
        let previous_lock = same_file::Handle::from_path(&lock)?;
        replace(&path)?;
        assert_eq!(
            same_file::Handle::from_path(&lock)?,
            previous_lock,
            "an ordinary replacement retains the existing receipt lock"
        );
        assert_eq!(
            fs::read(&path)?,
            PAYLOAD,
            "the replacement is byte-identical"
        );
        assert!(
            !owner.path().exists(),
            "the ordinary replacement retired the old proof"
        );
    }
    Ok(())
}

#[test]
fn ordinary_writers_leave_exactly_the_payload_in_a_fresh_destination() -> TestResult {
    for replace in ordinary_writers() {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("output.bin");
        replace(&path)?;
        let names = fs::read_dir(dir.path())?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        assert_eq!(names, [OsStr::new("output.bin")], "exact output inventory");
        assert_eq!(fs::read(&path)?, PAYLOAD);
    }
    Ok(())
}

fn ordinary_writers() -> [fn(&Path) -> TestResult; 3] {
    [write_bytes, write_counted, write_io]
}

fn write_bytes(path: &Path) -> TestResult {
    write_bytes_replacing(path, PAYLOAD)?;
    Ok(())
}

fn write_counted(path: &Path) -> TestResult {
    let result = write_replacing_counted(path, false, |writer| {
        writer
            .write_all(PAYLOAD)
            .map_err(|source| write_error(path, source))
    })?;
    assert!(result.changed, "a counted replacement occurred");
    Ok(())
}

fn write_io(path: &Path) -> TestResult {
    write_replacing_io_protected(path, None, None, |writer| writer.write_all(PAYLOAD))?;
    Ok(())
}

#[test]
fn ordinary_replacement_preserves_foreign_and_disjoint_companions() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("raw.bin");
    let owner = FileReceipt::new(&path, None, None)?;
    let aggregate = dir.path().join(".p4kx-origin.jsonl");
    fs::write(owner.path(), FOREIGN)?;
    fs::write(&aggregate, FOREIGN)?;
    write_bytes(&path)?;
    assert_eq!(fs::read(owner.path())?, FOREIGN, "foreign sibling kept");
    assert_eq!(
        fs::read(&aggregate)?,
        FOREIGN,
        "foreign directory metadata kept"
    );

    fs::remove_file(&aggregate)?;
    publish_directory(dir.path(), "other.bin")?;
    let previous = fs::read(&aggregate)?;
    write_bytes(&path)?;
    assert_eq!(
        fs::read(&aggregate)?,
        previous,
        "verified disjoint receipt kept"
    );
    write_io(&dir.path().join("other.bin"))?;
    assert!(
        !aggregate.exists(),
        "an actual overlapping receipt is retired"
    );
    assert_eq!(fs::read(owner.path())?, FOREIGN, "unrelated sibling kept");
    Ok(())
}

#[test]
fn a_failed_write_and_a_counted_noop_preserve_previous_bytes_and_receipt() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("target.bin");
    let owner = FileReceipt::new(&path, None, None)?;
    publish_file(&owner)?;
    let receipt = fs::read(owner.path())?;
    let failure = write_replacing_protected(&path, None, None, |writer| {
        writer
            .write_all(b"failed staged data")
            .map_err(|source| write_error(&path, source))?;
        Err::<(), _>(write_error(
            &path,
            io::Error::other("fixture write failure"),
        ))
    });
    assert!(failure.is_err(), "the injected staged write failed");
    assert_eq!(fs::read(&path)?, PAYLOAD, "old artifact");
    assert_eq!(fs::read(owner.path())?, receipt, "old companion");

    let same = write_replacing_counted(&path, true, |writer| {
        writer
            .write_all(PAYLOAD)
            .map_err(|source| write_error(&path, source))
    })?;
    assert!(!same.changed, "equal data did not replace the artifact");
    assert_eq!(
        fs::read(owner.path())?,
        receipt,
        "the same artifact keeps its proof"
    );
    Ok(())
}

#[test]
fn an_active_local_receipt_rejects_nested_ordinary_replacement() -> TestResult {
    let dir = tempfile::tempdir()?;
    let protected = dir.path().join("prior.bin");
    let owner = FileReceipt::new(&protected, None, None)?;
    publish_file(&owner)?;
    let previous = fs::read(owner.path())?;
    let other = FileReceipt::new(&dir.path().join("other.bin"), None, None)?;
    let inventory = SourceUses::new().freeze()?;
    other.publish(
        |writer| writer.write_all(PAYLOAD),
        |writer, binding, ()| {
            let nested = write_bytes_replacing(&protected, b"no nested replacement");
            assert!(
                matches!(nested, Err(crate::ConversionError::Write { source, .. })
                    if source.kind() == io::ErrorKind::WouldBlock),
                "the outer receipt holds the directory lock"
            );
            write_origin_companion(writer, binding, &inventory, [])
        },
    )?;
    assert_eq!(fs::read(&protected)?, PAYLOAD, "prior bytes");
    assert_eq!(fs::read(owner.path())?, previous, "prior proof");
    Ok(())
}

#[test]
fn independent_protected_replacements_in_one_directory_both_commit() -> TestResult {
    use crate::{ArchiveWriteProtection, CancelToken};
    use std::sync::Barrier;

    let dir = tempfile::tempdir()?;
    let source = dir.path().join("Data.p4k");
    fs::write(&source, b"synthetic archive identity")?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(&source, cancel.flag())?;
    let first = dir.path().join("first.bin");
    let second = dir.path().join("second.bin");
    let barrier = Barrier::new(2);
    let replace = |path: &Path| {
        write_replacing_protected(path, Some(&protection), Some(&cancel), |writer| {
            let write = writer
                .write_all(PAYLOAD)
                .map_err(|source| write_error(path, source));
            barrier.wait();
            write
        })
    };
    std::thread::scope(|scope| {
        let left = scope.spawn(|| replace(&first));
        let right = replace(&second);
        left.join().expect("ordinary replacement worker")?;
        right
    })?;
    assert_eq!(fs::read(&first)?, PAYLOAD, "first committed artifact");
    assert_eq!(fs::read(&second)?, PAYLOAD, "second committed artifact");
    let mut names = fs::read_dir(dir.path())?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<io::Result<Vec<_>>>()?;
    names.sort_unstable();
    assert_eq!(
        names,
        ["Data.p4k", "first.bin", "second.bin"],
        "exact concurrent inventory"
    );
    Ok(())
}

#[test]
fn an_unchanged_external_file_needs_no_publication_lock() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("unchanged.bin");
    fs::write(&path, PAYLOAD)?;
    let same = write_replacing_counted(&path, true, |writer| {
        writer
            .write_all(PAYLOAD)
            .map_err(|source| write_error(&path, source))
    })?;
    assert!(!same.changed, "the external file was left in place");
    assert_eq!(
        fs::read_dir(dir.path())?.count(),
        1,
        "no lock or temporary file"
    );
    Ok(())
}

#[test]
fn a_protected_archive_directory_gets_neither_output_nor_lock() -> TestResult {
    use crate::{ArchiveWriteProtection, CancelToken};

    let dir = tempfile::tempdir()?;
    fs::create_dir(dir.path().join("Bin64"))?;
    let source = dir.path().join("Data.p4k");
    fs::write(&source, b"synthetic archive identity")?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(&source, cancel.flag())?;
    let output = dir.path().join("report.bin");
    let result: Result<(), _> =
        write_replacing_protected(&output, Some(&protection), Some(&cancel), |_| {
            unreachable!("the protected directory is rejected before staging")
        });
    assert!(result.is_err(), "the protected destination is refused");
    let mut names = fs::read_dir(dir.path())?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<Vec<_>, _>>()?;
    names.sort_unstable();
    assert_eq!(
        names,
        [OsStr::new("Bin64"), OsStr::new("Data.p4k")],
        "the installation inventory is unchanged"
    );
    Ok(())
}

#[path = "file_receipt_lexical_tests.rs"]
mod lexical;
