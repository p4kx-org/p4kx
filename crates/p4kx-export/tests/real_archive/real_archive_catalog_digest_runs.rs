// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Seek as _, Write};
use std::path::Path;

use sha2::{Digest as _, Sha256};

pub(super) type Fingerprint = [u8; 32];

pub(super) const PAIR_BYTES: u64 = 64;
const BATCH_ENTRIES: usize = 16_384;
const MERGE_LEVELS: usize = u64::BITS as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OriginDigest {
    pub(super) id: Fingerprint,
    pub(super) payload: Fingerprint,
}

impl OriginDigest {
    pub(super) fn id_hash(id: &str) -> Fingerprint {
        Sha256::digest(id.as_bytes()).into()
    }

    pub(super) fn read(reader: &mut impl BufRead) -> io::Result<Option<Self>> {
        if reader.fill_buf()?.is_empty() {
            return Ok(None);
        }
        let mut id = Fingerprint::default();
        let mut payload = Fingerprint::default();
        reader.read_exact(&mut id)?;
        reader.read_exact(&mut payload)?;
        Ok(Some(Self { id, payload }))
    }

    fn write(self, writer: &mut impl Write) -> io::Result<()> {
        writer.write_all(&self.id)?;
        writer.write_all(&self.payload)
    }
}

pub(super) struct DigestRuns<'directory> {
    directory: &'directory Path,
    batch: Vec<OriginDigest>,
    levels: [Option<File>; MERGE_LEVELS],
    batch_entries: usize,
}

impl<'directory> DigestRuns<'directory> {
    pub(super) const fn new(directory: &'directory Path) -> Self {
        Self::with_batch_entries(directory, BATCH_ENTRIES)
    }

    const fn with_batch_entries(directory: &'directory Path, batch_entries: usize) -> Self {
        Self {
            directory,
            batch: Vec::new(),
            levels: [const { None }; MERGE_LEVELS],
            batch_entries,
        }
    }

    pub(super) fn push(&mut self, entry: OriginDigest) -> io::Result<()> {
        self.batch.push(entry);
        if self.batch.len() == self.batch_entries {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.batch.is_empty() {
            return Ok(());
        }
        self.batch.sort_unstable_by_key(|entry| entry.id);
        let mut writer = RunWriter::new(self.directory)?;
        for entry in self.batch.drain(..) {
            writer.push(entry)?;
        }
        let mut run = writer.finish()?;
        for level in &mut self.levels {
            let Some(previous) = level.take() else {
                *level = Some(run);
                return Ok(());
            };
            run = merge(self.directory, previous, run)?;
        }
        Err(io::Error::other(
            "source digest merge exceeded its run levels",
        ))
    }

    pub(super) fn finish(mut self) -> io::Result<File> {
        self.flush()?;
        let mut joined = None;
        for run in self.levels.into_iter().flatten() {
            joined = Some(match joined {
                Some(previous) => merge(self.directory, previous, run)?,
                None => run,
            });
        }
        joined.map_or_else(|| tempfile::tempfile_in(self.directory), Ok)
    }
}

struct RunWriter {
    output: BufWriter<File>,
    last: Option<OriginDigest>,
}

impl RunWriter {
    fn new(directory: &Path) -> io::Result<Self> {
        Ok(Self {
            output: BufWriter::new(tempfile::tempfile_in(directory)?),
            last: None,
        })
    }

    fn push(&mut self, entry: OriginDigest) -> io::Result<()> {
        if let Some(previous) = self.last {
            if previous.id > entry.id {
                return Err(io::Error::other("source digest run is out of order"));
            }
            if previous.id == entry.id {
                return if previous.payload == entry.payload {
                    Ok(())
                } else {
                    Err(io::Error::other(
                        "the SDK produced conflicting observations for one source ID",
                    ))
                };
            }
        }
        entry.write(&mut self.output)?;
        self.last = Some(entry);
        Ok(())
    }

    fn finish(self) -> io::Result<File> {
        let mut file = self
            .output
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?;
        file.rewind()?;
        Ok(file)
    }
}

fn merge(directory: &Path, left: File, right: File) -> io::Result<File> {
    let mut writer = RunWriter::new(directory)?;
    let mut left = BufReader::new(left);
    let mut right = BufReader::new(right);
    let mut first = OriginDigest::read(&mut left)?;
    let mut second = OriginDigest::read(&mut right)?;
    while let (Some(a), Some(b)) = (first, second) {
        if a.id <= b.id {
            writer.push(a)?;
            first = OriginDigest::read(&mut left)?;
        } else {
            writer.push(b)?;
            second = OriginDigest::read(&mut right)?;
        }
    }
    let (mut pending, tail) = if first.is_some() {
        (first, &mut left)
    } else {
        (second, &mut right)
    };
    while let Some(entry) = pending {
        writer.push(entry)?;
        pending = OriginDigest::read(tail)?;
    }
    writer.finish()
}

#[test]
fn digests_merge_sorted_passes_and_refuse_origin_conflicts() -> io::Result<()> {
    let scratch = tempfile::tempdir()?;
    let entry = |id, payload| OriginDigest {
        id: [id; 32],
        payload: [payload; 32],
    };
    let mut runs = DigestRuns::with_batch_entries(scratch.path(), 2);
    for pair in [
        entry(5, 50),
        entry(1, 10),
        entry(2, 20),
        entry(1, 10),
        entry(6, 60),
        entry(3, 30),
        entry(5, 50),
    ] {
        runs.push(pair)?;
    }
    let mut checked = BufReader::new(runs.finish()?);
    for expected in [
        entry(1, 10),
        entry(2, 20),
        entry(3, 30),
        entry(5, 50),
        entry(6, 60),
    ] {
        assert_eq!(
            OriginDigest::read(&mut checked)?,
            Some(expected),
            "binary runs merge and deduplicate in identifier order"
        );
    }
    assert_eq!(OriginDigest::read(&mut checked)?, None, "no extra records");

    let mut conflict = DigestRuns::with_batch_entries(scratch.path(), 1);
    conflict.push(entry(1, 50))?;
    assert!(
        conflict.push(entry(1, 60)).is_err(),
        "the same ID with different payloads conflicts across runs"
    );
    let mut duplicate = DigestRuns::with_batch_entries(scratch.path(), 3);
    duplicate.push(entry(1, 50))?;
    duplicate.push(entry(1, 60))?;
    assert!(
        duplicate.finish().is_err(),
        "the same ID with different payloads conflicts within a run"
    );
    let mut truncated = io::Cursor::new([0; 63]);
    assert!(
        OriginDigest::read(&mut truncated).is_err(),
        "truncated digest pairs are refused"
    );
    Ok(())
}
