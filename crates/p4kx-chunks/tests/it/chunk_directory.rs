// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::{self, Cursor, Read, Seek, SeekFrom};

use p4kx_chunks::{ChunkDirectory, ChunkDirectoryError, ChunkFile};
use p4kx_testkit::TestChunkFile;

type TestResult = Result<(), Box<dyn Error>>;

struct TableOnly {
    inner: Cursor<Vec<u8>>,
    table_end: u64,
}

impl Read for TableOnly {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.inner.position() + buffer.len() as u64 > self.table_end {
            return Err(io::Error::other("unexpected payload read"));
        }
        self.inner.read(buffer)
    }
}

impl Seek for TableOnly {
    fn seek(&mut self, at: SeekFrom) -> io::Result<u64> {
        self.inner.seek(at)
    }
}

#[test]
fn seekable_directory_uses_only_original_header_and_table() -> TestResult {
    for bytes in [
        TestChunkFile::ivo()
            .chunk(10, 0x900, [1; 1024])
            .chunk(11, 0x901, [2; 2048])
            .bytes(),
        TestChunkFile::crch()
            .chunk(0x1000, 1, [3; 16])
            .big_endian_chunk(0x1001, 2, [4; 8])
            .bytes(),
    ] {
        let expected = ChunkFile::parse(bytes.as_slice())?;
        let mut reader = TableOnly {
            inner: Cursor::new(bytes.clone()),
            table_end: expected.table_range().end,
        };
        let mut admitted = 0;
        let mut checked = 0;
        let directory = ChunkDirectory::read(
            &mut reader,
            bytes.len() as u64,
            |rows| {
                admitted = rows;
                Ok(())
            },
            || {
                checked += 1;
                Ok(())
            },
        )?;
        assert_eq!(admitted, 2, "admit the original validated row count");
        assert!(checked >= 2, "check each original table row");
        assert_eq!(directory.container(), expected.container());
        assert_eq!(directory.table_range(), expected.table_range());
        assert_eq!(directory.chunks(), expected.chunks());
        assert_eq!(directory.table_row(1), expected.table_row(1));
    }
    Ok(())
}

#[test]
fn admission_and_checkpoint_failures_remain_original_input_errors() {
    let bytes = TestChunkFile::ivo().chunk(10, 0x900, [1; 8]).bytes();
    let length = bytes.len() as u64;
    let mut input = Cursor::new(bytes);
    let refused = ChunkDirectory::read(
        &mut input,
        length,
        |_| Err(io::Error::other("admission denied")),
        || Ok(()),
    );
    assert!(
        matches!(refused, Err(ChunkDirectoryError::Input(error)) if error.to_string() == "admission denied")
    );
    assert_eq!(input.position(), 16, "no table I/O before admission");
    let interrupted = ChunkDirectory::read(
        &mut input,
        length,
        |_| Ok(()),
        || Err(io::Error::other("checkpoint stopped")),
    );
    assert!(
        matches!(interrupted, Err(ChunkDirectoryError::Input(error)) if error.to_string() == "checkpoint stopped")
    );
}
