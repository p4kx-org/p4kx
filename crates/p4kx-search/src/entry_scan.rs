// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Write};

use p4kx_extraction::cryxml::{self, Format};
use p4kx_extraction::{Archive, ArchiveError, Entry};

use crate::gate::Gate;
use crate::line_match::LineMatch;
use crate::line_sink::{Halt, LineSink};
use crate::{EntryHit, Needle};

const READ_LEN: usize = 64 << 10;
const HEAD_LEN: usize = 16;
const MAX_WHOLE_LEN: u64 = 32 << 20;
const DECODE_BUFFER_LEN: u64 = READ_LEN as u64 + 4;
const CRYXML_MEMORY_FACTOR: u64 = 6;
const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// What the search of one Entry came to.
#[derive(Debug)]
pub(crate) enum Outcome {
    Scanned(Vec<EntryHit>),
    Binary,
    TooLarge,
    KeyRequired,
    Unreadable,
    Cancelled,
}

/// What every Entry of a search shares.
pub(crate) struct Context<'a> {
    pub(crate) needle: &'a Needle,
    pub(crate) per_item: usize,
    pub(crate) max_len: u64,
    pub(crate) gate: &'a Gate,
    pub(crate) stop: &'a (dyn Fn() -> bool + Sync),
}

/// The ways an Entry's bytes become lines of text.
enum Form {
    Utf8,
    Utf16 { big_endian: bool },
    CryXml,
}

impl Form {
    fn of(head: &[u8]) -> Self {
        match head {
            [0xFF, 0xFE, ..] => Self::Utf16 { big_endian: false },
            [0xFE, 0xFF, ..] => Self::Utf16 { big_endian: true },
            start if matches!(cryxml::detect(start), Some(Format::CryXml(_))) => Self::CryXml,
            _ => Self::Utf8,
        }
    }

    const fn is_whole(&self) -> bool {
        !matches!(self, Self::Utf8)
    }

    /// Returns the bytes held in memory while an Entry of `size` bytes is searched: for UTF-16
    /// the Entry and the decoding buffer, for `CryXml` the Entry and what is derived from it.
    /// The parser refuses tables that overlap, so every byte of the file belongs to one table: a
    /// node byte becomes at most 1.5 bytes of nodes and links, an attribute byte 2, and a byte of
    /// string data at most 5, one for the copy that repairs invalid UTF-8 and four for the offset
    /// of a terminator.
    const fn memory(&self, size: u64) -> u64 {
        match self {
            Self::Utf8 => 0,
            Self::Utf16 { .. } => size.saturating_add(DECODE_BUFFER_LEN),
            Self::CryXml => size.saturating_mul(CRYXML_MEMORY_FACTOR),
        }
    }
}

/// Searches the lines of `entry`, reading plain text as a stream and `CryXml` and UTF-16 as a
/// whole under the memory budget of the gate. `buffer` is scratch space reused between Entries.
pub(crate) fn scan(
    archive: &Archive,
    entry: &Entry,
    context: &Context<'_>,
    buffer: &mut Vec<u8>,
) -> Outcome {
    let size = entry.uncompressed_size();
    if size > context.max_len {
        return Outcome::TooLarge;
    }
    if entry.is_encrypted() && !archive.has_key() {
        return Outcome::KeyRequired;
    }
    let mut reader = match archive.open_entry(entry) {
        Ok(reader) => reader.reject_overlong(),
        Err(error) => return opening_failed(&error),
    };
    let mut head = [0; HEAD_LEN];
    let Ok(head_len) = fill(&mut reader, &mut head) else {
        return Outcome::Unreadable;
    };
    let head = head.get(..head_len).unwrap_or_default();
    let form = Form::of(head);
    if form.is_whole() && size > MAX_WHOLE_LEN {
        return Outcome::TooLarge;
    }
    let memory = form
        .memory(size)
        .saturating_add(LineMatch::memory(context.needle.folded_len()));
    let _permit = context.gate.acquire(memory);
    let sink = LineSink::new(context.needle, entry.path(), context.per_item, context.stop);
    let result = match form {
        Form::Utf8 => stream(&mut reader, head, buffer, sink),
        Form::Utf16 { big_endian } => {
            whole(&mut reader, head, size).and_then(|data| utf16(&data, big_endian, sink))
        }
        Form::CryXml => whole(&mut reader, head, size).and_then(|data| cryxml_lines(&data, sink)),
    };
    match result {
        Ok((hits, None | Some(Halt::Done))) => Outcome::Scanned(hits),
        Ok((_, Some(Halt::Binary))) | Err(Failure::Binary) => Outcome::Binary,
        Ok((_, Some(Halt::Cancelled))) => Outcome::Cancelled,
        Ok((_, Some(Halt::Unreadable))) | Err(Failure::Read) => Outcome::Unreadable,
    }
}

enum Failure {
    Binary,
    Read,
}

type Searched = Result<(Vec<EntryHit>, Option<Halt>), Failure>;

fn opening_failed(error: &ArchiveError) -> Outcome {
    if error.is_key_required() || error.is_wrong_key() {
        Outcome::KeyRequired
    } else {
        Outcome::Unreadable
    }
}

fn stream(
    reader: &mut impl Read,
    head: &[u8],
    buffer: &mut Vec<u8>,
    mut sink: LineSink<'_>,
) -> Searched {
    buffer.resize(READ_LEN, 0);
    let start = head.strip_prefix(UTF8_BOM).unwrap_or(head);
    if sink.write_all(start).is_err() {
        return Ok(sink.finish());
    }
    loop {
        let read = reader.read(buffer).map_err(|_| Failure::Read)?;
        if read == 0 {
            return Ok(sink.finish());
        }
        if sink
            .write_all(buffer.get(..read).unwrap_or_default())
            .is_err()
        {
            return Ok(sink.finish());
        }
    }
}

fn whole(reader: &mut impl Read, head: &[u8], size: u64) -> Result<Vec<u8>, Failure> {
    let mut data = Vec::new();
    data.try_reserve_exact(usize::try_from(size).map_err(|_| Failure::Read)?)
        .map_err(|_| Failure::Read)?;
    data.extend_from_slice(head);
    reader
        .take(size.saturating_sub(head.len() as u64))
        .read_to_end(&mut data)
        .map_err(|_| Failure::Read)?;
    Ok(data)
}

fn cryxml_lines(data: &[u8], sink: LineSink<'_>) -> Searched {
    let document = cryxml::parse(data).map_err(|_| Failure::Read)?;
    let mut sink = sink.accepting_controls();
    let written = document.write_xml(&mut sink);
    if written.is_err() && sink.halt().is_none() {
        return Err(Failure::Read);
    }
    Ok(sink.finish())
}

fn utf16(data: &[u8], big_endian: bool, mut sink: LineSink<'_>) -> Searched {
    let (pairs, odd) = data.get(2..).unwrap_or_default().as_chunks::<2>();
    if !odd.is_empty() {
        return Err(Failure::Binary);
    }
    let unit = if big_endian {
        u16::from_be_bytes
    } else {
        u16::from_le_bytes
    };
    let mut text = String::with_capacity(READ_LEN + 4);
    for decoded in char::decode_utf16(pairs.iter().map(|&pair| unit(pair))) {
        let Ok(character) = decoded else {
            return Err(Failure::Binary);
        };
        text.push(character);
        if text.len() >= READ_LEN {
            if sink.write_all(text.as_bytes()).is_err() {
                return Ok(sink.finish());
            }
            text.clear();
        }
    }
    let _written = sink.write_all(text.as_bytes());
    Ok(sink.finish())
}

fn fill(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while let Some(rest) = buffer.get_mut(filled..)
        && !rest.is_empty()
    {
        match reader.read(rest)? {
            0 => break,
            read => filled += read,
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::{CRYXML_MEMORY_FACTOR, DECODE_BUFFER_LEN, Form};

    #[test]
    fn the_memory_of_an_entry_counts_what_is_derived_from_its_bytes() {
        assert_eq!(Form::Utf8.memory(1000), 0, "streamed text holds nothing");
        assert_eq!(
            Form::Utf16 { big_endian: false }.memory(1000),
            1000 + DECODE_BUFFER_LEN,
            "the Entry and the decoding buffer"
        );
        assert_eq!(
            Form::CryXml.memory(1000),
            1000 * CRYXML_MEMORY_FACTOR,
            "the Entry and its tables"
        );
        assert_eq!(Form::CryXml.memory(u64::MAX), u64::MAX, "saturates");
    }
}
