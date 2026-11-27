// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use crate::line_match::{LineError, LineMatch};
use crate::{EntryHit, Needle};

const STOP_POLL_BYTES: usize = 64 << 10;

/// Why a [`LineSink`] stopped taking data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Halt {
    Binary,
    Unreadable,
    Done,
    Cancelled,
}

#[derive(Clone, Copy)]
enum Kind {
    Text,
    ConvertedXml,
}

/// Searches complete source lines with bounded literal and excerpt state.
pub(crate) struct LineSink<'search> {
    matcher: LineMatch<'search>,
    path: &'search str,
    per_item: usize,
    kind: Kind,
    after_cr: bool,
    held_cr: bool,
    stop: &'search (dyn Fn() -> bool + Sync),
    line_len: u64,
    line_start: u64,
    line: u64,
    hits: Vec<EntryHit>,
    halt: Option<Halt>,
}

impl<'search> LineSink<'search> {
    pub(crate) fn new(
        needle: &'search Needle,
        path: &'search str,
        per_item: usize,
        stop: &'search (dyn Fn() -> bool + Sync),
    ) -> Self {
        Self {
            matcher: LineMatch::new(needle),
            path,
            per_item,
            kind: Kind::Text,
            after_cr: false,
            held_cr: false,
            stop,
            line_len: 0,
            line_start: 0,
            line: 0,
            hits: Vec::new(),
            halt: None,
        }
    }

    /// Accepts converted XML controls and treats only line feed as a line boundary.
    pub(crate) const fn accepting_controls(mut self) -> Self {
        self.kind = Kind::ConvertedXml;
        self
    }

    pub(crate) const fn halt(&self) -> Option<Halt> {
        self.halt
    }

    pub(crate) fn finish(mut self) -> (Vec<EntryHit>, Option<Halt>) {
        if self.halt.is_none() && self.line_len != 0 {
            let _ended = self.complete();
        }
        (self.hits, self.halt)
    }

    fn stopped(&mut self, halt: Halt) -> io::Error {
        self.halt = Some(halt);
        io::Error::other("the search of the Entry ended")
    }

    fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        if matches!(self.kind, Kind::Text) && bytes.iter().copied().any(is_binary) {
            return Err(self.stopped(Halt::Binary));
        }
        self.line_len += bytes.len() as u64;
        if bytes.is_empty() {
            return Ok(());
        }
        if self.held_cr {
            self.matcher
                .write(b"\r")
                .map_err(|error| self.failed(error))?;
            self.held_cr = false;
        }
        let bytes = if matches!(self.kind, Kind::ConvertedXml) {
            bytes.strip_suffix(b"\r").map_or(bytes, |head| {
                self.held_cr = true;
                head
            })
        } else {
            bytes
        };
        self.matcher
            .write(bytes)
            .map_err(|error| self.failed(error))
    }

    fn failed(&mut self, error: LineError) -> io::Error {
        self.stopped(match error {
            LineError::Binary => Halt::Binary,
            LineError::MissingMatchRange => Halt::Unreadable,
        })
    }

    fn complete(&mut self) -> io::Result<()> {
        self.held_cr = false;
        let excerpt = self.matcher.finish().map_err(|error| self.failed(error))?;
        self.matcher.reset();
        self.line += 1;
        if let Some(excerpt) = excerpt {
            self.hits.push(EntryHit {
                path: self.path.to_owned(),
                line: self.line,
                offset: self.line_start,
                excerpt,
            });
            if self.hits.len() >= self.per_item {
                return Err(self.stopped(Halt::Done));
            }
        }
        Ok(())
    }

    fn block(&mut self, mut rest: &[u8]) -> io::Result<()> {
        if self.after_cr {
            self.after_cr = false;
            if let Some(tail) = rest.strip_prefix(b"\n") {
                rest = tail;
                self.line_start += 1;
            }
        }
        let cr_breaks = matches!(self.kind, Kind::Text);
        while let Some(newline) = rest
            .iter()
            .position(|&byte| byte == b'\n' || (cr_breaks && byte == b'\r'))
        {
            let (line, after) = rest.split_at(newline);
            let mut taken = 1;
            if after.first() == Some(&b'\r') {
                match after.get(1) {
                    Some(b'\n') => taken = 2,
                    None => self.after_cr = true,
                    Some(_) => {}
                }
            }
            rest = after.get(taken..).unwrap_or_default();
            self.append(line)?;
            self.complete()?;
            self.line_start += self.line_len + taken as u64;
            self.line_len = 0;
        }
        self.append(rest)
    }
}

impl Write for LineSink<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if let Some(halt) = self.halt {
            return Err(self.stopped(halt));
        }
        for block in buffer.chunks(STOP_POLL_BYTES) {
            if (self.stop)() {
                return Err(self.stopped(Halt::Cancelled));
            }
            self.block(block)?;
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Returns whether a byte is a control character absent from plain text.
pub(crate) const fn is_binary(byte: u8) -> bool {
    (byte < 0x20 && !matches!(byte, b'\t' | b'\n' | 0x0C | b'\r')) || byte == 0x7F
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::Write as _;

    use super::LineSink;
    use crate::Needle;

    fn places(writes: &[&[u8]]) -> Result<Vec<(u64, u64)>, Box<dyn Error>> {
        let needle = Needle::new("needle")?;
        let mut sink = LineSink::new(&needle, "a.txt", 10, &|| false);
        for write in writes {
            sink.write_all(write)?;
        }
        let (hits, _) = sink.finish();
        Ok(hits.iter().map(|hit| (hit.line, hit.offset)).collect())
    }

    #[test]
    fn a_carriage_return_and_line_feed_split_between_writes_are_one_line_break()
    -> Result<(), Box<dyn Error>> {
        assert_eq!(
            places(&[b"ab\r", b"\nneedle\r", b"needle\n"])?,
            [(2, 4), (3, 11)],
            "one break after ab, a lone break before the last line"
        );
        Ok(())
    }

    #[test]
    fn a_carriage_return_stays_in_a_line_of_converted_xml() -> Result<(), Box<dyn Error>> {
        let needle = Needle::new("needle")?;
        let mut sink = LineSink::new(&needle, "a.xml", 10, &|| false).accepting_controls();
        sink.write_all(b"a\rb needle\n")?;
        let (hits, _) = sink.finish();
        assert_eq!(
            hits.iter().map(|hit| hit.line).collect::<Vec<_>>(),
            [1],
            "one line"
        );
        Ok(())
    }
}
