// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Write};

use p4kx_archive::Checksum;
use p4kx_cryxml::{self as cryxml, Format};

use crate::ExtractionError;
use crate::buffers::Buffers;
use crate::source::SourceEntry;
use crate::write::{Chunk, Destination, EntryWriter, Outcome, write_error};

/// The number of bytes read from the start of an Entry to recognise `CryXml`.
pub(crate) const PEEK_LEN: usize = 16;
/// The largest `CryXml` Entry a Conversion reads into memory.
pub(crate) const MAX_CONVERTED_LEN: u64 = 1 << 30;
const READ_LEN: usize = 1 << 20;
const RESERVED_LEN: usize = 16 << 20;

/// Returns whether data starting with `start` is `CryXml`; text XML and other data are not.
pub(crate) fn is_cryxml(start: &[u8]) -> bool {
    matches!(cryxml::detect(start), Some(Format::CryXml(_)))
}

impl EntryWriter<'_> {
    /// Reads up to [`PEEK_LEN`] bytes of the Entry into `prefix`; returns how many, or `None`
    /// once the run is to stop.
    pub(crate) fn peek(
        &self,
        reader: &mut impl Read,
        prefix: &mut [u8; PEEK_LEN],
        written: &mut u64,
    ) -> Result<Option<usize>, ExtractionError> {
        let mut filled = 0;
        while let Some(rest) = prefix.get_mut(filled..)
            && !rest.is_empty()
        {
            match self
                .read_chunk(reader, rest)
                .map_err(ExtractionError::read)?
            {
                Chunk::Data(count) => {
                    filled += count;
                    self.count(count, written);
                }
                Chunk::End => break,
                Chunk::Cancelled => return Ok(None),
            }
        }
        Ok(Some(filled))
    }

    /// Reads the rest of a `CryXml` Entry that begins with `start`, converts it to XML and writes
    /// that under the Conflict policy, comparing an existing file with the XML.
    pub(crate) fn convert(
        &self,
        entry: &SourceEntry<'_>,
        reader: &mut impl Read,
        start: &[u8],
        buffers: &mut Buffers,
        destination: &Destination,
        written: &mut u64,
    ) -> Result<Outcome, ExtractionError> {
        let size = entry.uncompressed_size();
        let len = usize::try_from(size)
            .ok()
            .filter(|_| size <= MAX_CONVERTED_LEN)
            .ok_or(ExtractionError::TooLargeToConvert {
                size,
                limit: MAX_CONVERTED_LEN,
            })?;
        let data = &mut buffers.data;
        data.clear();
        data.reserve(len.min(RESERVED_LEN));
        data.extend_from_slice(start);
        if matches!(
            self.read_rest(reader, data, len, written)?,
            Chunk::Cancelled
        ) {
            return Ok(Outcome::Cancelled);
        }
        let document = cryxml::parse(data).map_err(ExtractionError::Convert)?;
        let xml = &mut buffers.output;
        xml.clear();
        document
            .write_xml(&mut *xml)
            .map_err(|source| write_error(destination.file.clone(), source))?;
        let expected = || (xml.len() as u64, Checksum::Crc32c(crc32c::crc32c(xml)));
        if let Some(outcome) = self.decide(destination, expected, &mut buffers.chunk)? {
            return Ok(outcome);
        }
        if self.tracker.should_stop() {
            return Ok(Outcome::Cancelled);
        }
        let (temporary, mut file, ownership) = self.temporary(&destination.folder)?;
        let copied = file
            .write_all(xml)
            .map(|()| Chunk::End)
            .map_err(|source| write_error(destination.file.clone(), source));
        drop(file);
        self.put_in_place(
            &temporary,
            &ownership,
            &destination.file,
            copied,
            Outcome::Converted,
        )
    }

    /// Appends the Entry data after what `data` holds until the end, reading straight into
    /// `data` and growing it no further than the data that arrives.
    fn read_rest(
        &self,
        reader: &mut impl Read,
        data: &mut Vec<u8>,
        len: usize,
        written: &mut u64,
    ) -> Result<Chunk, ExtractionError> {
        loop {
            let start = data.len();
            let want = (len - start).clamp(1, READ_LEN);
            data.resize(start + want, 0);
            let chunk = data.get_mut(start..).map_or(Ok(Chunk::End), |rest| {
                self.read_chunk(reader, rest).map_err(ExtractionError::read)
            });
            match chunk {
                Ok(Chunk::Data(count)) if start + count <= len => {
                    data.truncate(start + count);
                    self.count(count, written);
                }
                Ok(Chunk::Data(_)) => return Err(longer_than_declared()),
                other => {
                    data.truncate(start);
                    return other;
                }
            }
        }
    }

    fn count(&self, count: usize, written: &mut u64) {
        *written += count as u64;
        self.tracker.wrote(count as u64);
    }
}

fn longer_than_declared() -> ExtractionError {
    ExtractionError::Read(io::Error::new(
        io::ErrorKind::InvalidData,
        "the Entry holds more data than its size",
    ))
}
