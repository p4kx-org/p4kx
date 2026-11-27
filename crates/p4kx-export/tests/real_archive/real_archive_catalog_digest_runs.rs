// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read as _, Seek as _, Write};
use std::path::Path;

pub(super) type Fingerprint = [u8; 32];

const BATCH_BYTES: usize = 1024 * 1024;
const MERGE_LEVELS: usize = u64::BITS as usize;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct OriginDigest {
    pub(super) id: Box<str>,
    pub(super) payload: Fingerprint,
}

impl OriginDigest {
    fn read(reader: &mut impl BufRead) -> io::Result<Option<Self>> {
        if reader.fill_buf()?.is_empty() {
            return Ok(None);
        }
        let mut length = [0; 8];
        reader.read_exact(&mut length)?;
        let length = u64::from_be_bytes(length);
        let mut id = Vec::new();
        let read = io::copy(&mut reader.take(length), &mut id)?;
        if read != length {
            return Err(io::Error::other("partial source digest identifier"));
        }
        let id = String::from_utf8(id).map_err(io::Error::other)?;
        let mut payload = Fingerprint::default();
        reader.read_exact(&mut payload)?;
        Ok(Some(Self {
            id: id.into_boxed_str(),
            payload,
        }))
    }

    fn write(&self, writer: &mut impl Write) -> io::Result<()> {
        let length = u64::try_from(self.id.len()).map_err(io::Error::other)?;
        writer.write_all(&length.to_be_bytes())?;
        writer.write_all(self.id.as_bytes())?;
        writer.write_all(&self.payload)
    }

    fn batch_bytes(&self) -> usize {
        size_of::<Self>().saturating_add(self.id.len())
    }
}

pub(super) struct DigestRuns<'directory> {
    directory: &'directory Path,
    batch: Vec<OriginDigest>,
    batch_bytes: usize,
    levels: [Option<DigestRun>; MERGE_LEVELS],
    batch_limit: usize,
}

impl<'directory> DigestRuns<'directory> {
    pub(super) const fn new(directory: &'directory Path) -> Self {
        Self::with_batch_limit(directory, BATCH_BYTES)
    }

    const fn with_batch_limit(directory: &'directory Path, batch_limit: usize) -> Self {
        Self {
            directory,
            batch: Vec::new(),
            batch_bytes: 0,
            levels: [const { None }; MERGE_LEVELS],
            batch_limit,
        }
    }

    pub(super) fn push(&mut self, entry: OriginDigest) -> io::Result<()> {
        self.batch_bytes = self.batch_bytes.saturating_add(entry.batch_bytes());
        self.batch.push(entry);
        if self.batch_bytes >= self.batch_limit {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.batch.is_empty() {
            return Ok(());
        }
        self.batch
            .sort_unstable_by(|left, right| left.id.cmp(&right.id));
        let mut writer = RunWriter::new(self.directory)?;
        for entry in self.batch.drain(..) {
            writer.push(entry)?;
        }
        self.batch_bytes = 0;
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

    pub(super) fn finish(mut self) -> io::Result<DigestRun> {
        self.flush()?;
        let mut joined = None;
        for run in self.levels.into_iter().flatten() {
            joined = Some(match joined {
                Some(previous) => merge(self.directory, previous, run)?,
                None => run,
            });
        }
        match joined {
            Some(run) => Ok(run),
            None => RunWriter::new(self.directory)?.finish(),
        }
    }
}

pub(super) struct DigestRun {
    file: File,
    count: u64,
}

impl DigestRun {
    pub(super) const fn count(&self) -> u64 {
        self.count
    }

    pub(super) fn cursor(&self) -> io::Result<DigestCursor> {
        let mut file = self.file.try_clone()?;
        file.rewind()?;
        let mut input = BufReader::new(file);
        let next = OriginDigest::read(&mut input)?;
        Ok(DigestCursor {
            input,
            next,
            previous: String::new(),
        })
    }
}

pub(super) struct DigestCursor {
    input: BufReader<File>,
    next: Option<OriginDigest>,
    previous: String,
}

impl DigestCursor {
    pub(super) fn observe(&mut self, id: &str) -> io::Result<Option<Fingerprint>> {
        if self.previous.as_str() >= id {
            return Err(io::Error::other(
                "Catalog source observation IDs are unordered or duplicated",
            ));
        }
        self.previous.clear();
        self.previous.push_str(id);
        let Some(expected) = &self.next else {
            return Ok(None);
        };
        match expected.id.as_ref().cmp(id) {
            Ordering::Greater => Ok(None),
            Ordering::Less => Err(io::Error::other(
                "missing Catalog source observation needed by the SDK",
            )),
            Ordering::Equal => {
                let payload = expected.payload;
                self.next = OriginDigest::read(&mut self.input)?;
                Ok(Some(payload))
            }
        }
    }
}

struct RunWriter {
    output: BufWriter<File>,
    last: Option<OriginDigest>,
    count: u64,
}

impl RunWriter {
    fn new(directory: &Path) -> io::Result<Self> {
        Ok(Self {
            output: BufWriter::new(tempfile::tempfile_in(directory)?),
            last: None,
            count: 0,
        })
    }

    fn push(&mut self, entry: OriginDigest) -> io::Result<()> {
        if let Some(previous) = &self.last {
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
        self.count = self
            .count
            .checked_add(1)
            .ok_or_else(|| io::Error::other("source digest count overflow"))?;
        Ok(())
    }

    fn finish(self) -> io::Result<DigestRun> {
        let mut file = self
            .output
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?;
        file.rewind()?;
        Ok(DigestRun {
            file,
            count: self.count,
        })
    }
}

fn merge(directory: &Path, left: DigestRun, right: DigestRun) -> io::Result<DigestRun> {
    let mut writer = RunWriter::new(directory)?;
    let mut left = BufReader::new(left.file);
    let mut right = BufReader::new(right.file);
    let mut first = OriginDigest::read(&mut left)?;
    let mut second = OriginDigest::read(&mut right)?;
    loop {
        let (pending, input) = match (&first, &second) {
            (Some(a), Some(b)) if a.id <= b.id => (&mut first, &mut left),
            (Some(_), None) => (&mut first, &mut left),
            (_, Some(_)) => (&mut second, &mut right),
            (None, None) => break,
        };
        if let Some(entry) = pending.take() {
            writer.push(entry)?;
        }
        *pending = OriginDigest::read(input)?;
    }
    writer.finish()
}

#[test]
fn digests_merge_sorted_passes_and_refuse_origin_conflicts() -> io::Result<()> {
    let scratch = tempfile::tempdir()?;
    let entry = |id: &str, payload| OriginDigest {
        id: id.into(),
        payload: [payload; 32],
    };
    let one = entry("a", 10).batch_bytes();
    let mut runs = DigestRuns::with_batch_limit(scratch.path(), 2 * one);
    for pair in [
        entry("e", 50),
        entry("a", 10),
        entry("b", 20),
        entry("a", 10),
        entry("f", 60),
        entry("c", 30),
        entry("e", 50),
    ] {
        runs.push(pair)?;
    }
    let run = runs.finish()?;
    assert_eq!(run.count(), 5, "duplicate SDK observations collapse");
    let mut cursor = run.cursor()?;
    for (id, payload) in [("a", 10), ("b", 20), ("c", 30), ("e", 50), ("f", 60)] {
        assert_eq!(
            cursor.observe(id)?,
            Some([payload; 32]),
            "binary runs merge and deduplicate in identifier order"
        );
    }

    let mut conflict = DigestRuns::with_batch_limit(scratch.path(), one);
    conflict.push(entry("a", 50))?;
    assert!(
        conflict.push(entry("a", 60)).is_err(),
        "the same ID with different payloads conflicts across runs"
    );
    let mut duplicate = DigestRuns::with_batch_limit(scratch.path(), 3 * one);
    duplicate.push(entry("a", 50))?;
    duplicate.push(entry("a", 60))?;
    assert!(
        duplicate.finish().is_err(),
        "the same ID with different payloads conflicts within a run"
    );
    let mut truncated = io::Cursor::new(46_u64.to_be_bytes());
    assert!(
        OriginDigest::read(&mut truncated).is_err(),
        "truncated digest pairs are refused"
    );
    Ok(())
}
