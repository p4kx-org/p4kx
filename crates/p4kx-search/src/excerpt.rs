// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::ops::Range;

pub(crate) const CONTEXT_BEFORE: usize = 32;
pub(crate) const CONTEXT_AFTER: usize = 64;
pub(crate) const MAX_MATCH_CHARS: usize = 96;
const ELLIPSIS: char = '\u{2026}';
const REPLACEMENT: char = '\u{fffd}';

/// Borrowed original text and boundary cuts for one excerpt.
pub(crate) struct ExcerptParts<'text> {
    pub(crate) head: &'text str,
    pub(crate) found: &'text str,
    pub(crate) tail: &'text str,
    pub(crate) head_cut: bool,
    pub(crate) tail_cut: bool,
}

/// A short piece of text around a match, safe to print: control characters and bidirectional
/// controls are replaced, and a cut at either end is marked with an ellipsis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Excerpt {
    text: String,
    matched: Range<usize>,
}

impl Excerpt {
    /// Returns the text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the byte range of the match within [`text`](Self::text).
    #[must_use]
    pub fn matched(&self) -> Range<usize> {
        self.matched.clone()
    }

    /// Returns the text before the match.
    #[must_use]
    pub fn before(&self) -> &str {
        self.text.get(..self.matched.start).unwrap_or_default()
    }

    /// Returns the matched text.
    #[must_use]
    pub fn matched_text(&self) -> &str {
        self.text.get(self.matched.clone()).unwrap_or_default()
    }

    /// Returns the text after the match.
    #[must_use]
    pub fn after(&self) -> &str {
        self.text.get(self.matched.end..).unwrap_or_default()
    }

    pub(crate) fn around(line: &str, matched: Range<usize>) -> Self {
        let head = line.get(..matched.start).unwrap_or_default();
        let found = line.get(matched.clone()).unwrap_or_default();
        let tail = line.get(matched.end..).unwrap_or_default();
        let (found, found_cut) = leading(found, MAX_MATCH_CHARS);
        let (head, head_cut) = trailing(head, CONTEXT_BEFORE);
        let (tail, tail_cut) = if found_cut {
            ("", false)
        } else {
            leading(tail, CONTEXT_AFTER)
        };
        Self::from_parts(&ExcerptParts {
            head,
            found,
            tail,
            head_cut,
            tail_cut: found_cut || tail_cut,
        })
    }

    pub(crate) fn from_parts(parts: &ExcerptParts<'_>) -> Self {
        let ExcerptParts {
            head,
            found,
            tail,
            head_cut,
            tail_cut,
        } = *parts;
        let mut text = String::with_capacity(head.len() + found.len() + tail.len() + 6);
        if head_cut {
            text.push(ELLIPSIS);
        }
        push_safe(&mut text, head);
        let start = text.len();
        push_safe(&mut text, found);
        let end = text.len();
        push_safe(&mut text, tail);
        if tail_cut {
            text.push(ELLIPSIS);
        }
        Self {
            text,
            matched: start..end,
        }
    }
}

/// Returns `text` as one printable line.
///
/// Line breaks and tabs become a space; other control characters and bidirectional controls
/// become U+FFFD, so that text from an Archive cannot move the cursor, recolour the terminal or
/// reorder what follows.
#[must_use]
pub fn display_text(text: &str) -> Cow<'_, str> {
    if !text.chars().any(is_unsafe) {
        return Cow::Borrowed(text);
    }
    let mut safe = String::with_capacity(text.len());
    push_safe(&mut safe, text);
    Cow::Owned(safe)
}

fn push_safe(out: &mut String, text: &str) {
    out.extend(text.chars().map(|c| match c {
        '\t' | '\n' | '\r' => ' ',
        c if is_unsafe(c) => REPLACEMENT,
        c => c,
    }));
}

const fn is_unsafe(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{00ad}'
                | '\u{061c}'
                | '\u{180e}'
                | '\u{200b}'..='\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{feff}'
        )
}

fn leading(text: &str, chars: usize) -> (&str, bool) {
    match text.char_indices().nth(chars) {
        Some((end, _)) => (text.get(..end).unwrap_or_default(), true),
        None => (text, false),
    }
}

fn trailing(text: &str, chars: usize) -> (&str, bool) {
    match text.char_indices().rev().nth(chars.saturating_sub(1)) {
        Some((start, _)) if start > 0 => (text.get(start..).unwrap_or_default(), true),
        _ => (text, false),
    }
}
