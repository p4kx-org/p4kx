// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::{RefCell, RefMut};
use std::io::{self, Read};

use p4kx_texture::SiblingReader;

use crate::texture_parts::{PartReader, Parts};
use crate::textures::FilePartReads;

/// The parts of a texture, keeping a copy of one part's data as it is read.
pub(crate) struct Capturing<'p, 'a> {
    pub(crate) parts: &'p Parts<'a>,
    pub(crate) capture: Option<u32>,
    pub(crate) data: RefCell<Vec<u8>>,
    pub(crate) file_reads: Option<&'p RefCell<FilePartReads>>,
}

impl SiblingReader for Capturing<'_, '_> {
    type Part<'r>
        = CapturingReader<'r>
    where
        Self: 'r;

    fn part_len(&self, index: u32) -> io::Result<Option<u64>> {
        self.parts.part_len(index)
    }

    fn open_part(&self, index: u32) -> io::Result<Self::Part<'_>> {
        let sink = (self.capture == Some(index))
            .then(|| self.data.try_borrow_mut().ok())
            .flatten()
            .map(|mut data| {
                data.clear();
                data
            });
        Ok(CapturingReader {
            inner: self.parts.open_part(index)?,
            sink,
            index,
            file_reads: self.file_reads,
        })
    }
}

/// The data of one part, appended to the captured data when it is the captured part.
pub(crate) struct CapturingReader<'r> {
    inner: PartReader<'r>,
    sink: Option<RefMut<'r, Vec<u8>>>,
    index: u32,
    file_reads: Option<&'r RefCell<FilePartReads>>,
}

impl Read for CapturingReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buffer)?;
        if let Some(sink) = &mut self.sink {
            sink.extend_from_slice(buffer.get(..count).unwrap_or_default());
        }
        Ok(count)
    }
}

impl Drop for CapturingReader<'_> {
    fn drop(&mut self) {
        if let Some(reads) = self.file_reads {
            reads
                .borrow_mut()
                .record(self.index, self.inner.file_origin());
        }
    }
}

/// The parts of a texture with one part served from data captured earlier instead of read again.
pub(crate) struct Replay<'p, 'a> {
    pub(crate) parts: &'p Parts<'a>,
    pub(crate) captured: Option<(u32, &'p [u8])>,
}

impl SiblingReader for Replay<'_, '_> {
    type Part<'r>
        = ReplayReader<'r>
    where
        Self: 'r;

    fn part_len(&self, index: u32) -> io::Result<Option<u64>> {
        match self.captured {
            Some((captured, data)) if captured == index => Ok(Some(data.len() as u64)),
            _ => self.parts.part_len(index),
        }
    }

    fn open_part(&self, index: u32) -> io::Result<Self::Part<'_>> {
        match self.captured {
            Some((captured, data)) if captured == index => Ok(ReplayReader::Captured(data)),
            _ => self.parts.open_part(index).map(ReplayReader::Part),
        }
    }
}

/// The data of one part: captured earlier or read from where it is stored.
pub(crate) enum ReplayReader<'r> {
    Captured(&'r [u8]),
    Part(PartReader<'r>),
}

impl Read for ReplayReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Captured(data) => data.read(buffer),
            Self::Part(reader) => reader.read(buffer),
        }
    }
}
