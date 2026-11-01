// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use super::FileSource;

const DATA: &[u8] = b"0123456789abcdef";

fn archive_file() -> io::Result<(tempfile::TempDir, PathBuf)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    fs::write(&path, DATA)?;
    Ok((dir, path))
}

fn read(source: &FileSource, offset: u64, len: usize) -> io::Result<Vec<u8>> {
    let mut buffer = vec![0; len];
    source.read_into(offset, &mut buffer)?;
    Ok(buffer)
}

#[test]
fn a_read_returns_the_bytes_at_the_offset() -> io::Result<()> {
    let (_dir, path) = archive_file()?;
    let source = FileSource::open(&path)?;

    assert_eq!(source.len(), DATA.len() as u64);
    assert_eq!(read(&source, 4, 6)?, b"456789");
    assert_eq!(read(&source, 0, DATA.len())?, DATA, "the whole file");
    Ok(())
}

#[test]
fn a_read_to_the_end_succeeds_and_one_byte_more_is_an_unexpected_eof() -> io::Result<()> {
    let (_dir, path) = archive_file()?;
    let source = FileSource::open(&path)?;
    let end = DATA.len() as u64;

    assert_eq!(read(&source, end - 3, 3)?, b"def");
    assert_eq!(read(&source, end, 0)?, b"", "nothing at the end");
    for (offset, len) in [(end - 3, 4), (end, 1), (end + 10, 1), (0, DATA.len() + 1)] {
        let error = read(&source, offset, len).expect_err("past the end");
        assert_eq!(
            error.kind(),
            io::ErrorKind::UnexpectedEof,
            "offset {offset}, {len} bytes"
        );
    }
    assert_eq!(read(&source, 0, 2)?, b"01", "the source stays usable");
    Ok(())
}

#[test]
fn another_handle_holding_the_file_open_does_not_block_reading() -> io::Result<()> {
    let (_dir, path) = archive_file()?;
    let writer = OpenOptions::new().read(true).write(true).open(&path)?;

    let source = FileSource::open(&path)?;

    assert_eq!(read(&source, 8, 4)?, b"89ab");
    drop(writer);
    assert_eq!(read(&source, 0, 4)?, b"0123");
    Ok(())
}

#[test]
fn concurrent_readers_read_correctly_while_another_handle_holds_the_file() -> io::Result<()> {
    let (_dir, path) = archive_file()?;
    let _writer = OpenOptions::new().write(true).open(&path)?;
    let source = FileSource::open(&path)?;

    std::thread::scope(|scope| {
        let readers: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    (0..200).try_for_each(|_| {
                        assert_eq!(read(&source, 10, 6)?, b"abcdef");
                        Ok::<(), io::Error>(())
                    })
                })
            })
            .collect();
        for reader in readers {
            let result = reader.join().expect("a reader thread panicked");
            assert!(result.is_ok(), "{result:?}");
        }
    });
    Ok(())
}

#[test]
fn a_file_that_grew_is_refused_for_a_new_handle_and_still_read_through_the_open_one()
-> io::Result<()> {
    let (_dir, path) = archive_file()?;
    let source = FileSource::open(&path)?;
    OpenOptions::new()
        .append(true)
        .open(&path)?
        .write_all(b"more")?;

    assert_eq!(read(&source, 0, 4)?, b"0123", "the pooled handle");
    let pooled = source.take_handle()?;
    let refused = source.take_handle().expect_err("a new handle");
    assert_eq!(refused.kind(), io::ErrorKind::ResourceBusy);
    source.return_handle(pooled);
    Ok(())
}
