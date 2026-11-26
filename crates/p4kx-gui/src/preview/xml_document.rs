// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use p4kx_extraction::XmlRenderOrigins;

use super::XmlPreviewOriginError;
use crate::comparison::shown_len;

/// The most bytes of converted XML a preview keeps; the lines after them are left out.
pub const XML_TEXT_LIMIT: usize = 64 * 1024 * 1024;

static SERIAL: AtomicU64 = AtomicU64::new(0);

/// An XML text split into lines for a view that lays out only the lines it shows.
#[derive(Debug)]
pub struct XmlDocument {
    serial: u64,
    bytes: Vec<u8>,
    starts: Vec<u32>,
    truncated: bool,
    widest: usize,
    provenance: Option<Result<XmlRenderOrigins, XmlPreviewOriginError>>,
}

impl XmlDocument {
    /// Splits `bytes` into lines; `truncated` says whether lines were left out after them.
    ///
    /// Text past [`XML_TEXT_LIMIT`] is cut at the last line break before it.
    #[must_use]
    pub fn new(mut bytes: Vec<u8>, truncated: bool) -> Self {
        let truncated = truncated || bytes.len() > XML_TEXT_LIMIT;
        if truncated {
            cut(&mut bytes, XML_TEXT_LIMIT);
        }
        let mut starts = Vec::with_capacity(bytes.len() / 32 + 1);
        let mut widest = (0, 0);
        let mut start = 0;
        for line in bytes.split_inclusive(|&byte| byte == b'\n') {
            let index = starts.len();
            starts.push(u32::try_from(start).unwrap_or(u32::MAX));
            let width = shown_len(line);
            if width > widest.0 {
                widest = (width, index);
            }
            start += line.len();
        }
        bytes.shrink_to_fit();
        starts.shrink_to_fit();
        Self {
            serial: SERIAL.fetch_add(1, Ordering::Relaxed),
            bytes,
            starts,
            truncated,
            widest: widest.1,
            provenance: None,
        }
    }

    pub(super) fn with_provenance(
        mut self,
        provenance: Result<XmlRenderOrigins, XmlPreviewOriginError>,
    ) -> Self {
        self.provenance = Some(provenance);
        self
    }

    /// Returns the source evidence for the retained converted spans, when collected.
    ///
    /// # Errors
    /// Returns an error if the converted bytes were displayed without their source proof.
    #[must_use]
    pub const fn provenance(&self) -> Option<Result<&XmlRenderOrigins, &XmlPreviewOriginError>> {
        match &self.provenance {
            Some(proof) => Some(proof.as_ref()),
            None => None,
        }
    }

    /// Returns a number no other XML document of this process has.
    #[must_use]
    pub const fn serial(&self) -> u64 {
        self.serial
    }

    /// Returns the whole text.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns whether lines were left out after the text.
    #[must_use]
    pub const fn is_truncated(&self) -> bool {
        self.truncated
    }

    /// Returns the number of lines.
    #[must_use]
    pub const fn line_count(&self) -> usize {
        self.starts.len()
    }

    /// Returns the zero-based line `index` with its terminator, empty past the end.
    #[must_use]
    pub fn line(&self, index: usize) -> &[u8] {
        self.line_range(index)
            .and_then(|bytes| self.bytes.get(bytes))
            .unwrap_or_default()
    }

    pub(crate) fn line_range(&self, index: usize) -> Option<Range<usize>> {
        let start = *self.starts.get(index)? as usize;
        let end = self
            .starts
            .get(index + 1)
            .map_or(self.bytes.len(), |&at| at as usize);
        Some(start..end)
    }

    /// Returns the longest line by its characters, a tab counting as four.
    #[must_use]
    pub const fn widest(&self) -> usize {
        self.widest
    }
}

/// Cuts `bytes` after the last line break within its first `limit` bytes.
fn cut(bytes: &mut Vec<u8>, limit: usize) {
    let head = bytes.get(..limit).unwrap_or(bytes);
    let keep = head
        .iter()
        .rposition(|&byte| byte == b'\n')
        .map_or(0, |at| at + 1);
    bytes.truncate(keep);
}

/// Collects written XML up to a limit, and stops the writer once it is reached or once the
/// preview the XML is for was dropped.
pub(super) struct CappedWriter<'a> {
    pub(super) out: Vec<u8>,
    limit: usize,
    pub(super) capped: bool,
    dropped: &'a AtomicBool,
}

impl<'a> CappedWriter<'a> {
    pub(super) const fn new(limit: usize, dropped: &'a AtomicBool) -> Self {
        Self {
            out: Vec::new(),
            limit,
            capped: false,
            dropped,
        }
    }
}

impl Write for CappedWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.dropped.load(Ordering::Acquire) {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        }
        let room = self.limit.saturating_sub(self.out.len());
        if buf.len() > room {
            self.out
                .extend_from_slice(buf.get(..room).unwrap_or_default());
            self.capped = true;
            return Err(io::Error::from(io::ErrorKind::WriteZero));
        }
        self.out.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::sync::atomic::AtomicBool;

    use super::{CappedWriter, XmlDocument, cut};

    fn lines(text: &XmlDocument) -> Vec<&[u8]> {
        (0..text.line_count())
            .map(|index| text.line(index))
            .collect()
    }

    #[test]
    fn the_text_is_split_after_each_line_break() {
        let text = XmlDocument::new(b"<a>\n  <b/>\n</a>".to_vec(), false);

        assert_eq!(lines(&text), [&b"<a>\n"[..], b"  <b/>\n", b"</a>"], "lines");
        assert!(text.line(3).is_empty(), "past the end");
        assert_eq!(text.widest(), 1, "the indented line is the widest");
        assert!(!text.is_truncated(), "complete");
    }

    #[test]
    fn an_empty_text_has_no_lines() {
        assert_eq!(XmlDocument::new(Vec::new(), false).line_count(), 0, "lines");
    }

    #[test]
    fn a_long_text_is_cut_at_a_line_break() {
        let mut bytes = b"<a>\n<b/>\n<c/>\n".to_vec();

        cut(&mut bytes, 11);

        assert_eq!(bytes, b"<a>\n<b/>\n", "the line cut through is left out");
    }

    #[test]
    fn the_writer_stops_at_its_limit() {
        let dropped = AtomicBool::new(false);
        let mut writer = CappedWriter::new(6, &dropped);

        assert!(writer.write_all(b"<a>\n").is_ok(), "fits");
        assert!(writer.write_all(b"<b/>\n").is_err(), "over the limit");
        assert!(writer.capped, "capped");

        let text = XmlDocument::new(writer.out, writer.capped);
        assert_eq!(lines(&text), [b"<a>\n"], "only whole lines");
        assert!(text.is_truncated(), "truncated");
    }

    #[test]
    fn the_writer_stops_once_the_preview_was_dropped() {
        let dropped = AtomicBool::new(true);
        let mut writer = CappedWriter::new(64, &dropped);

        assert!(writer.write_all(b"<a/>").is_err(), "stopped");
        assert!(!writer.capped, "not capped");
    }
}
