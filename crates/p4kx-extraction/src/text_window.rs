// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_cryxml as cryxml;

use crate::entry_content::EntryReadError;

/// Counts every byte written and keeps those in `[offset, offset + keep)`; writing more than
/// `limit` bytes in all fails.
#[derive(Debug)]
pub(crate) struct Window {
    offset: u64,
    keep: u64,
    limit: u64,
    written: u64,
    kept: Vec<u8>,
    exceeded: bool,
}

impl Window {
    /// Creates an empty window.
    pub(crate) fn new(offset: u64, keep: usize, limit: u64) -> Self {
        Self {
            offset,
            keep: keep as u64,
            limit,
            written: 0,
            kept: Vec::with_capacity(keep.min(1 << 20)),
            exceeded: false,
        }
    }

    /// Returns the number of bytes written.
    pub(crate) const fn written(&self) -> u64 {
        self.written
    }

    /// Returns whether a write went beyond the limit.
    pub(crate) const fn exceeded(&self) -> bool {
        self.exceeded
    }

    /// Returns the bytes kept.
    pub(crate) fn into_kept(self) -> Vec<u8> {
        self.kept
    }
}

impl Write for Window {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let end = self.written.saturating_add(buffer.len() as u64);
        if end > self.limit {
            self.exceeded = true;
            return Err(io::Error::other("the text exceeds its size limit"));
        }
        let from = self.written.max(self.offset);
        let to = end.min(self.offset.saturating_add(self.keep));
        if from < to {
            let start = usize::try_from(from - self.written).unwrap_or(usize::MAX);
            let stop = usize::try_from(to - self.written).unwrap_or(usize::MAX);
            self.kept
                .extend_from_slice(buffer.get(start..stop).unwrap_or_default());
        }
        self.written = end;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Converts the `CryXml` in `data` to XML, keeping `keep` bytes of it from `offset`.
///
/// # Errors
///
/// Returns [`EntryReadError::Convert`] for corrupt `CryXml` and [`EntryReadError::TooLarge`] when
/// the XML exceeds `limit` bytes.
pub(crate) fn convert_into_window(
    data: &[u8],
    offset: u64,
    keep: usize,
    limit: u64,
    observe: impl FnMut(cryxml::XmlWriteField) -> io::Result<()>,
) -> Result<Window, EntryReadError> {
    let document = cryxml::parse(data).map_err(EntryReadError::Convert)?;
    let mut window = Window::new(offset, keep, limit);
    match document.write_xml_observed(&mut window, observe) {
        Ok(()) => Ok(window),
        Err(_) if window.exceeded() => Err(EntryReadError::TooLarge { limit }),
        Err(source) => Err(EntryReadError::Read(source)),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

    use super::{EntryReadError, Window, convert_into_window};

    #[test]
    fn a_window_keeps_its_range_across_writes_and_counts_everything()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut window = Window::new(3, 4, 100);
        window.write_all(b"ab")?;
        window.write_all(b"cdef")?;
        window.write_all(b"ghij")?;
        assert_eq!(window.written(), 10, "every byte counted");
        assert_eq!(window.into_kept(), b"defg", "bytes 3 to 6");
        Ok(())
    }

    #[test]
    fn writing_beyond_the_limit_fails() {
        let mut window = Window::new(0, 4, 5);
        assert!(window.write_all(b"abc").is_ok(), "within the limit");
        assert!(window.write_all(b"def").is_err(), "beyond the limit");
        assert!(window.exceeded(), "marked as exceeded");
    }

    #[test]
    fn converted_xml_above_the_limit_is_too_large() {
        let cryxml = TestElement::new("Ship")
            .attribute("name", "Gladius")
            .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
        assert!(
            matches!(
                convert_into_window(&cryxml, 0, 8, 16, |_| Ok(())),
                Err(EntryReadError::TooLarge { limit: 16 })
            ),
            "the XML is longer than 16 bytes"
        );
        let window = convert_into_window(&cryxml, 0, 8, 1 << 20, |_| Ok(()));
        assert!(
            window.is_ok_and(|window| window.written() > 16),
            "a larger limit converts it"
        );
    }
}
