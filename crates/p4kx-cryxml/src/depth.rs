// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

/// The deepest element nesting a reader of untrusted text XML accepts: deeper documents are
/// refused before a recursive parser or reader sees them.
pub const MAX_TEXT_DEPTH: usize = 128;

/// A text XML document whose elements nest deeper than [`MAX_TEXT_DEPTH`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("its elements are nested deeper than {limit}")]
pub struct TextTooDeep {
    limit: usize,
}

impl TextTooDeep {
    /// Returns the nesting that is allowed.
    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit
    }
}

/// Returns an error when the elements of the text XML `text` nest deeper than
/// [`MAX_TEXT_DEPTH`], before any parser recurses that deep.
///
/// The scan is linear in `text`, does not allocate and skips comments, CDATA sections and
/// processing instructions, so a closing tag hidden in one does not lower the depth. A
/// declaration is skipped up to its first `>`; documents with a DOCTYPE internal subset are
/// refused by the parsers that read them, not by this scan.
///
/// # Errors
///
/// Returns [`TextTooDeep`] when an element opens at a depth above the limit.
pub fn check_text_depth(text: &str) -> Result<(), TextTooDeep> {
    check_text_depth_with_limit(text, MAX_TEXT_DEPTH)
}

pub(crate) fn check_text_depth_with_limit(text: &str, limit: usize) -> Result<(), TextTooDeep> {
    let bytes = text.as_bytes();
    let mut depth = 0_usize;
    let mut at = 0;
    while let Some(offset) = bytes
        .get(at..)
        .and_then(|rest| rest.iter().position(|b| *b == b'<'))
    {
        at = at.saturating_add(offset);
        let rest = bytes.get(at..).unwrap_or_default();
        if rest.starts_with(b"<!--") {
            at = skip_past(bytes, at.saturating_add(4), b"-->");
        } else if rest.starts_with(b"<![CDATA[") {
            at = skip_past(bytes, at.saturating_add(9), b"]]>");
        } else if rest.starts_with(b"<?") {
            at = skip_past(bytes, at.saturating_add(2), b"?>");
        } else if rest.starts_with(b"<!") {
            at = skip_past(bytes, at.saturating_add(2), b">");
        } else {
            let end = tag_end(bytes, at);
            let tag = bytes.get(at..end).unwrap_or_default();
            if tag.starts_with(b"</") {
                depth = depth.saturating_sub(1);
            } else if !tag.ends_with(b"/>") {
                depth = depth.saturating_add(1);
                if depth > limit {
                    return Err(TextTooDeep { limit });
                }
            }
            at = end;
        }
    }
    Ok(())
}

fn skip_past(bytes: &[u8], from: usize, needle: &[u8]) -> usize {
    bytes
        .get(from..)
        .and_then(|rest| {
            rest.windows(needle.len())
                .position(|window| window == needle)
        })
        .map_or(bytes.len(), |found| {
            from.saturating_add(found).saturating_add(needle.len())
        })
}

fn tag_end(bytes: &[u8], from: usize) -> usize {
    let mut quote = None;
    for (offset, byte) in bytes.iter().enumerate().skip(from.saturating_add(1)) {
        match (quote, *byte) {
            (None, b'"' | b'\'') => quote = Some(*byte),
            (Some(open), b) if b == open => quote = None,
            (None, b'>') => return offset.saturating_add(1),
            _ => {}
        }
    }
    bytes.len()
}
