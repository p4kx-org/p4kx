// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::read_at::ReadAt;

const BLOCK_LEN: usize = 1024;
const CARRY_LEN: usize = 3;

/// The observed UTF-8 validity of text bytes before the first NUL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WemTextUtf8 {
    /// Every observed byte is in the ASCII range.
    Ascii,
    /// The bytes are valid UTF-8 and include non-ASCII bytes.
    Valid,
    /// The observed bytes are not valid UTF-8.
    Invalid,
}

/// Text attached to a cue by a RIFF label or note.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WemCueText {
    /// Original cue identifier at the beginning of the subchunk body.
    pub cue_id: u32,
    /// Complete stored text region, including any terminator and residual bytes.
    pub raw_range: Range<u64>,
    /// Bytes before the first NUL, or the complete region when there is no NUL.
    pub text_range: Range<u64>,
    /// Original position of the first NUL byte when one occurs.
    pub terminator_offset: Option<u64>,
    /// Byte validity, independent of an authored character encoding.
    pub utf8: WemTextUtf8,
    /// Original bytes supplied to the UTF-8 validator before it stopped.
    pub utf8_input_range: Range<u64>,
}

impl WemCueText {
    pub(crate) fn read<I: ReadAt>(
        input: &mut I,
        cue_id: u32,
        raw_range: Range<u64>,
    ) -> Result<Self, I::Error> {
        let scan = scan_zstr(input, &raw_range)?;
        Ok(Self {
            cue_id,
            text_range: raw_range.start..scan.terminator.unwrap_or(raw_range.end),
            utf8_input_range: raw_range.start..scan.utf8_end,
            raw_range,
            terminator_offset: scan.terminator,
            utf8: scan.utf8,
        })
    }
}

struct TextScan {
    terminator: Option<u64>,
    utf8: WemTextUtf8,
    utf8_end: u64,
}

fn scan_zstr<I: ReadAt>(input: &mut I, range: &Range<u64>) -> Result<TextScan, I::Error> {
    let mut buffer = [0; BLOCK_LEN + CARRY_LEN];
    let mut scan = Utf8Scan {
        carry: 0,
        state: WemTextUtf8::Ascii,
        input_end: range.start,
    };
    let mut offset = range.start;
    while offset < range.end {
        let len = usize::try_from(range.end - offset)
            .unwrap_or(BLOCK_LEN)
            .min(BLOCK_LEN);
        let bytes = buffer.split_at_mut(scan.carry).1.split_at_mut(len).0;
        input.read_at(offset, bytes)?;
        let nul = bytes.iter().position(|&byte| byte == 0);
        let observed = nul.unwrap_or(len);
        scan.observe(&mut buffer, observed, offset + observed as u64);
        if let Some(nul) = nul {
            return Ok(scan.finish(Some(offset + nul as u64)));
        }
        offset += len as u64;
    }
    Ok(scan.finish(None))
}

struct Utf8Scan {
    carry: usize,
    state: WemTextUtf8,
    input_end: u64,
}

impl Utf8Scan {
    fn observe(&mut self, buffer: &mut [u8], len: usize, end: u64) {
        if self.state == WemTextUtf8::Invalid {
            return;
        }
        self.input_end = end;
        let prefix_len = self.carry + len;
        let prefix = buffer.split_at(prefix_len).0;
        match std::str::from_utf8(prefix) {
            Ok(text) => {
                self.carry = 0;
                if !text.is_ascii() {
                    self.state = WemTextUtf8::Valid;
                }
            }
            Err(error) if error.error_len().is_none() => {
                self.carry = prefix_len - error.valid_up_to();
                self.state = WemTextUtf8::Valid;
                buffer.copy_within(error.valid_up_to()..prefix_len, 0);
            }
            Err(_) => {
                self.carry = 0;
                self.state = WemTextUtf8::Invalid;
            }
        }
    }

    const fn finish(self, terminator: Option<u64>) -> TextScan {
        let utf8 = if self.carry == 0 {
            self.state
        } else {
            WemTextUtf8::Invalid
        };
        TextScan {
            terminator,
            utf8,
            utf8_end: self.input_end,
        }
    }
}
