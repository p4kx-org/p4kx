// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek, SeekFrom};
use std::ops::Range;

use crate::{AudioError, WemReadError};

pub(crate) trait ReadAt {
    type Error: From<AudioError>;

    fn byte_length(&self) -> u64;

    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), Self::Error>;
}

pub(crate) struct SliceSource<'bytes>(pub(crate) &'bytes [u8]);

impl ReadAt for SliceSource<'_> {
    type Error = AudioError;

    fn byte_length(&self) -> u64 {
        self.0.len() as u64
    }

    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), AudioError> {
        let bytes = usize::try_from(offset)
            .ok()
            .and_then(|start| self.0.get(start..))
            .and_then(|tail| tail.get(..output.len()))
            .ok_or_else(|| AudioError::Truncated {
                len: self.byte_length(),
                needed: offset.saturating_add(output.len() as u64),
            })?;
        output.copy_from_slice(bytes);
        Ok(())
    }
}

pub(crate) struct SeekSource<R> {
    reader: R,
    byte_length: u64,
}

impl<R> SeekSource<R> {
    pub(crate) const fn new(reader: R, byte_length: u64) -> Self {
        Self {
            reader,
            byte_length,
        }
    }

    pub(crate) const fn source_length(&self) -> u64 {
        self.byte_length
    }
}

impl<R: Read + Seek> SeekSource<R> {
    pub(crate) fn window(&mut self, range: Range<u64>) -> io::Result<impl Read + Seek + '_> {
        if !(range.start..=self.byte_length).contains(&range.end) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the byte window exceeds the source extent",
            ));
        }
        Ok(SeekWindow {
            source: self,
            range,
            position: 0,
        })
    }

    pub(crate) fn read_io_at(&mut self, offset: u64, output: &mut [u8]) -> io::Result<()> {
        if !output.is_empty() {
            self.reader.seek(SeekFrom::Start(offset))?;
            self.reader.read_exact(output)?;
        }
        Ok(())
    }
}

impl<R: Read + Seek> ReadAt for SeekSource<R> {
    type Error = WemReadError;

    fn byte_length(&self) -> u64 {
        self.byte_length
    }

    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), WemReadError> {
        let needed = offset.saturating_add(output.len() as u64);
        if needed > self.byte_length {
            return Err(AudioError::Truncated {
                len: self.byte_length,
                needed,
            }
            .into());
        }
        self.read_io_at(offset, output)?;
        Ok(())
    }
}

struct SeekWindow<'source, R> {
    source: &'source mut SeekSource<R>,
    range: Range<u64>,
    position: u64,
}

impl<R> SeekWindow<'_, R> {
    const fn length(&self) -> u64 {
        self.range.end - self.range.start
    }
}

impl<R: Read + Seek> Read for SeekWindow<'_, R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let remaining = self.length() - self.position;
        let count = output
            .len()
            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let output = output.get_mut(..count).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "the byte window buffer is invalid",
            )
        })?;
        self.source
            .read_io_at(self.range.start + self.position, output)?;
        self.position += count as u64;
        Ok(count)
    }
}

impl<R> Seek for SeekWindow<'_, R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let length = self.length();
        let position = match from {
            SeekFrom::Start(position) => i128::from(position),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
            SeekFrom::End(offset) => i128::from(length) + i128::from(offset),
        };
        self.position = u64::try_from(position)
            .ok()
            .filter(|position| *position <= length)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "the seek position is outside the byte window",
                )
            })?;
        Ok(self.position)
    }
}
