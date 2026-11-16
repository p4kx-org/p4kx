// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::str;

use p4kx_extraction::{FieldOrigin, FieldOrigins, SourceUses};

use crate::decode::DecodeOutcome;
use crate::line_diff_side::SideOrigins;
use crate::report_origins::{ReportOriginsError, put};
use crate::{Encoding, LineKind, TextDiff};

pub(crate) fn write(
    fields: &mut FieldOrigins,
    text: &TextDiff,
    sides: [&SideOrigins<'_>; 2],
    alignment: &FieldOrigin,
    uses: &SourceUses,
) -> Result<(), ReportOriginsError> {
    let [old_side, new_side] = sides;
    let mut old = TextPositions::new(old_side.outcome, text.old_bytes());
    let mut new = TextPositions::new(new_side.outcome, text.new_bytes());
    for (hunk_index, hunk) in text.hunks().enumerate() {
        let path = format!("/hunks/{hunk_index}");
        put(fields, &path, alignment.clone())?;
        for key in ["old_start", "old_lines", "new_start", "new_lines", "lines"] {
            put(fields, &format!("{path}/{key}"), alignment.clone())?;
        }
        for (line_index, line) in hunk.lines().enumerate() {
            let path = format!("{path}/lines/{line_index}");
            let text = match line.kind() {
                LineKind::Context | LineKind::Removed => {
                    old_side.line(old.original(line.bytes()), uses)?
                }
                LineKind::Added => new_side.line(new.original(line.bytes()), uses)?,
            };
            put(
                fields,
                &path,
                uses.origin_with_origins([alignment, &text], true)?,
            )?;
            put(fields, &format!("{path}/kind"), alignment.clone())?;
            put(fields, &format!("{path}/text"), text)?;
        }
    }
    Ok(())
}

struct TextPositions<'a> {
    outcome: Option<DecodeOutcome>,
    decoded: &'a [u8],
    utf16_at: usize,
    original_at: u64,
}

impl<'a> TextPositions<'a> {
    const fn new(outcome: Option<DecodeOutcome>, decoded: &'a [u8]) -> Self {
        Self {
            outcome,
            decoded,
            utf16_at: 0,
            original_at: 2,
        }
    }

    fn original(&mut self, line: &[u8]) -> Option<Range<u64>> {
        if matches!(self.outcome, Some(DecodeOutcome::Text(Encoding::CryXml(_)))) {
            return None;
        }
        let start = line
            .as_ptr()
            .addr()
            .checked_sub(self.decoded.as_ptr().addr())?;
        let end = start.checked_add(line.len())?;
        self.decoded.get(start..end)?;
        match self.outcome {
            Some(DecodeOutcome::Text(Encoding::Text)) => Some(start as u64..end as u64),
            Some(DecodeOutcome::Text(Encoding::Utf16Le | Encoding::Utf16Be)) => {
                let start = self.advance_utf16(start)?;
                let end = self.advance_utf16(end)?;
                Some(start..end)
            }
            _ => None,
        }
    }

    fn advance_utf16(&mut self, at: usize) -> Option<u64> {
        let skipped = str::from_utf8(self.decoded.get(self.utf16_at..at)?).ok()?;
        self.original_at += (skipped.encode_utf16().count() as u64) * 2;
        self.utf16_at = at;
        Some(self.original_at)
    }
}
