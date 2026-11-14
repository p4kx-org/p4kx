// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::ops::ControlFlow;

pub(crate) const PATCH_TAIL_BYTES: u64 = 16 << 20;
pub(crate) const PATCH_LINE_BYTES: usize = 64 << 10;
const LIBRARY_LINE_BYTES: usize = 1 << 20;
const READ_BUFFER: usize = 64 << 10;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    Whole,
    Tail,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadEnd {
    Complete,
    Stopped,
}

pub(crate) fn for_each_opened(
    mut file: impl Read + Seek,
    length: u64,
    part: Part,
    visit: impl FnMut(bool, &[u8]) -> ControlFlow<()>,
) -> io::Result<ReadEnd> {
    let tail_start = length.saturating_sub(PATCH_TAIL_BYTES);
    let (start, skipping) = if part == Part::Tail && tail_start > 0 {
        file.seek(SeekFrom::Start(tail_start - 1))?;
        let mut before = [0_u8; 1];
        file.read_exact(&mut before)?;
        (tail_start, before != *b"\n")
    } else {
        file.seek(SeekFrom::Start(0))?;
        (0, false)
    };
    let maximum = match part {
        Part::Whole => LIBRARY_LINE_BYTES,
        Part::Tail => PATCH_LINE_BYTES,
    };
    let input = file.take(length.saturating_sub(start));
    lines(input, start, length, tail_start, maximum, skipping, visit)
}

fn lines(
    input: impl Read,
    start: u64,
    length: u64,
    tail_start: u64,
    maximum: usize,
    mut skipping: bool,
    mut visit: impl FnMut(bool, &[u8]) -> ControlFlow<()>,
) -> io::Result<ReadEnd> {
    let mut reader = BufReader::with_capacity(READ_BUFFER, input);
    let mut line = Vec::new();
    let mut line_start = start;
    let mut position = start;
    loop {
        let buffer = match reader.fill_buf() {
            Ok(buffer) => buffer,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if buffer.is_empty() {
            if position != length {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "launcher log was truncated",
                ));
            }
            if !skipping && emit(line_start, tail_start, &line, &mut visit).is_break() {
                return Ok(ReadEnd::Stopped);
            }
            return Ok(ReadEnd::Complete);
        }
        let newline = buffer.iter().position(|&byte| byte == b'\n');
        let chunk = newline.map_or(buffer, |end| buffer.get(..end).unwrap_or_default());
        let consumed = newline.map_or(buffer.len(), |end| end + 1);
        if !skipping {
            if chunk.len() > maximum.saturating_sub(line.len()) {
                skipping = true;
                line.clear();
            } else {
                line.extend_from_slice(chunk);
            }
        }
        reader.consume(consumed);
        position += u64::try_from(consumed).map_err(io::Error::other)?;
        if newline.is_some() {
            if !skipping && emit(line_start, tail_start, &line, &mut visit).is_break() {
                return Ok(ReadEnd::Stopped);
            }
            line.clear();
            skipping = false;
            line_start = position;
        }
    }
}

fn emit(
    start: u64,
    tail_start: u64,
    line: &[u8],
    visit: &mut impl FnMut(bool, &[u8]) -> ControlFlow<()>,
) -> ControlFlow<()> {
    if line.is_empty() {
        return ControlFlow::Continue(());
    }
    visit(
        start >= tail_start,
        line.strip_suffix(b"\r").unwrap_or(line),
    )
}
