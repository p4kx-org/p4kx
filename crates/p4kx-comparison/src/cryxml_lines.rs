// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;

use p4kx_cryxml::{Document, XmlChildAbsence, XmlRawSpan, XmlRenderSpan};

use crate::text_diff::rendered_range;
use crate::{CancelToken, LineDiffError, LineKind, TextDiff};

/// The emitted hunk lines and their corresponding CryXml input intervals.
#[derive(Clone, Default)]
pub(crate) struct CryXmlLines {
    lines: Vec<CryXmlLine>,
}

#[derive(Clone)]
pub(crate) struct CryXmlLine {
    rendered: Range<u64>,
    covered: u64,
    complete: bool,
    pub(crate) facts: Vec<XmlRawSpan>,
    pub(crate) child_absences: Vec<XmlChildAbsence>,
}

impl CryXmlLine {
    const fn new(rendered: Range<u64>) -> Self {
        Self {
            covered: rendered.start,
            rendered,
            complete: true,
            facts: Vec::new(),
            child_absences: Vec::new(),
        }
    }

    fn observe(&mut self, span: &XmlRenderSpan) {
        let start = span.range().start.max(self.rendered.start);
        let end = span.range().end.min(self.rendered.end);
        let child_absence = span.child_absence();
        self.complete &= self.covered >= start && (span.complete() || child_absence.is_some());
        self.covered = self.covered.max(end);
        self.facts.extend(span.sources().cloned());
        self.child_absences.extend(child_absence.cloned());
    }

    pub(crate) const fn complete(&self) -> bool {
        self.complete && self.covered == self.rendered.end
    }
}

impl CryXmlLines {
    pub(crate) fn find(&self, range: Range<usize>) -> Option<&CryXmlLine> {
        let start = range.start as u64;
        let index = self
            .lines
            .binary_search_by_key(&start, |line| line.rendered.start)
            .ok()?;
        self.lines
            .get(index)
            .filter(|line| line.rendered.end == range.end as u64)
    }

    fn observe(&mut self, span: &XmlRenderSpan, first: &mut usize) {
        let rendered = span.range();
        while self
            .lines
            .get(*first)
            .is_some_and(|line| line.rendered.end <= rendered.start)
        {
            *first += 1;
        }
        for line in self.lines.iter_mut().skip(*first) {
            let after = if rendered.is_empty() {
                line.rendered.start > rendered.start
            } else {
                line.rendered.start >= rendered.end
            };
            if after {
                break;
            }
            line.observe(span);
        }
    }
}

#[derive(Clone, Copy)]
enum Side {
    Old,
    New,
}

impl Side {
    fn bytes(self, text: &TextDiff) -> &[u8] {
        match self {
            Self::Old => text.old_bytes(),
            Self::New => text.new_bytes(),
        }
    }

    const fn supplies(self, kind: LineKind) -> bool {
        matches!(
            (self, kind),
            (Self::Old, LineKind::Context | LineKind::Removed) | (Self::New, LineKind::Added)
        )
    }
}

pub(crate) fn capture(
    text: &TextDiff,
    documents: [Option<&Document<'_>>; 2],
    cancel: Option<&CancelToken>,
) -> Result<[CryXmlLines; 2], LineDiffError> {
    let [old, new] = documents;
    let (old, new) = rayon::join(
        || capture_side(text, old, Side::Old, cancel),
        || capture_side(text, new, Side::New, cancel),
    );
    Ok([old?, new?])
}

fn capture_side(
    text: &TextDiff,
    document: Option<&Document<'_>>,
    side: Side,
    cancel: Option<&CancelToken>,
) -> Result<CryXmlLines, LineDiffError> {
    let Some(document) = document else {
        return Ok(CryXmlLines::default());
    };
    let bytes = side.bytes(text);
    let mut evidence = CryXmlLines {
        lines: text
            .hunks()
            .flat_map(|hunk| hunk.lines())
            .filter(|line| side.supplies(line.kind()))
            .filter_map(|line| rendered_range(bytes, line.bytes()))
            .map(|range| CryXmlLine::new(range.start as u64..range.end as u64))
            .collect(),
    };
    if evidence.lines.is_empty() {
        return Ok(evidence);
    }
    let mut replay = Replay {
        remaining: bytes,
        cancel,
    };
    let mut first = 0;
    let result = document.write_xml_positioned(&mut replay, |span| {
        evidence.observe(&span, &mut first);
        Ok(())
    });
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(LineDiffError::Cancelled);
    }
    if result.is_ok() && replay.remaining.is_empty() {
        Ok(evidence)
    } else {
        Ok(CryXmlLines::default())
    }
}

/// Admits positioned facts only when the serializer emits the compared bytes.
struct Replay<'text, 'cancel> {
    remaining: &'text [u8],
    cancel: Option<&'cancel CancelToken>,
}

impl Write for Replay<'_, '_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(io::Error::other(LineDiffError::Cancelled));
        }
        self.remaining = self
            .remaining
            .strip_prefix(bytes)
            .ok_or(io::ErrorKind::InvalidData)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
