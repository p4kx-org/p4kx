// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use rayon::prelude::*;

use crate::decode::{DecodeOutcome, Decoded, Encoding, decode};
use crate::path_key::eq_normalised;
use crate::{Archive, CancelToken, ChangeKind, Entry, LineDiffError, TextDiff};

/// Why an Entry pair has no line diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotText {
    /// A side holds a NUL byte, or UTF-16 that does not decode, so it is binary.
    Binary,
    /// A side, or the XML converted from it, is larger than
    /// [`MAX_DIFF_LEN`](crate::MAX_DIFF_LEN) or has more than
    /// [`MAX_DIFF_LINES`](crate::MAX_DIFF_LINES) lines.
    TooLarge,
}

/// What the line diff of an Entry pair holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffBody {
    /// Both sides are text; a missing side is empty.
    Text(TextDiff),
    /// A side is not diffable line by line.
    NotText(NotText),
}

/// The line diff of one Entry between an old and a new Archive.
#[derive(Clone)]
pub struct LineDiff<'a> {
    old: LineInput<'a>,
    new: LineInput<'a>,
    body: DiffBody,
}

#[derive(Clone, Copy)]
pub(crate) struct LineInput<'a> {
    pub(crate) archive: &'a Archive,
    pub(crate) entry: Option<&'a Entry>,
    pub(crate) outcome: Option<DecodeOutcome>,
}

impl LineInput<'_> {
    const fn encoding(self) -> Option<Encoding> {
        match self.outcome {
            Some(DecodeOutcome::Text(encoding)) => Some(encoding),
            _ => None,
        }
    }
}

impl fmt::Debug for LineDiff<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LineDiff")
            .field("old", &self.old.entry)
            .field("new", &self.new.entry)
            .field("old_encoding", &self.old_encoding())
            .field("new_encoding", &self.new_encoding())
            .field("body", &self.body)
            .finish()
    }
}

impl PartialEq for LineDiff<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.old.entry == other.old.entry
            && self.new.entry == other.new.entry
            && self.old_encoding() == other.old_encoding()
            && self.new_encoding() == other.new_encoding()
            && self.body == other.body
    }
}

impl Eq for LineDiff<'_> {}

impl<'a> LineDiff<'a> {
    pub(crate) const fn original_inputs(&self) -> [LineInput<'a>; 2] {
        [self.old, self.new]
    }

    /// Returns the Entry path as the new side stores it, or as the old side does for a removed
    /// Entry.
    #[must_use]
    pub fn path(&self) -> &'a str {
        self.new.entry.or(self.old.entry).map_or("", Entry::path)
    }

    /// Returns how the Entry differs by its central-directory size and CRC-32C, `None` when it
    /// is unchanged.
    #[must_use]
    pub fn kind(&self) -> Option<ChangeKind> {
        match (self.old.entry, self.new.entry) {
            (Some(old), Some(new)) => (old.uncompressed_size() != new.uncompressed_size()
                || old.checksum() != new.checksum())
            .then_some(ChangeKind::Changed),
            (Some(_), None) => Some(ChangeKind::Removed),
            (None, _) => Some(ChangeKind::Added),
        }
    }

    /// Returns the Entry of the old side, `None` when it was added.
    #[must_use]
    pub const fn old_entry(&self) -> Option<&'a Entry> {
        self.old.entry
    }

    /// Returns the Entry of the new side, `None` when it was removed.
    #[must_use]
    pub const fn new_entry(&self) -> Option<&'a Entry> {
        self.new.entry
    }

    /// Returns how the old side was read, `None` when it is missing or not text.
    #[must_use]
    pub const fn old_encoding(&self) -> Option<Encoding> {
        self.old.encoding()
    }

    /// Returns how the new side was read, `None` when it is missing or not text.
    #[must_use]
    pub const fn new_encoding(&self) -> Option<Encoding> {
        self.new.encoding()
    }

    /// Returns the hunks, or why there are none.
    #[must_use]
    pub const fn body(&self) -> &DiffBody {
        &self.body
    }

    /// Returns the hunks, or why there are none, owned.
    #[must_use]
    pub fn into_body(self) -> DiffBody {
        self.body
    }

    /// Returns whether the Entry differs: added, removed, changed in size or CRC-32C, or with
    /// differing lines; an Entry whose lines are equal after conversion still differs when its
    /// bytes changed.
    #[must_use]
    pub fn has_differences(&self) -> bool {
        self.kind().is_some()
            || matches!(&self.body, DiffBody::Text(text) if text.has_differences())
    }
}

/// Returns the line diff of the Entry at `path` between the `old` and the `new` Archive, with
/// `context` unchanged lines around each change.
///
/// The path matches with either separator and ignoring ASCII case. `CryXml` is converted to XML
/// first, UTF-16 text with a byte order mark is decoded strictly, and other text is compared byte
/// for byte; a side with a NUL byte, or UTF-16 that does not decode, is binary. A side larger than
/// [`MAX_DIFF_LEN`](crate::MAX_DIFF_LEN) or with more than
/// [`MAX_DIFF_LINES`](crate::MAX_DIFF_LINES) lines is too large. A missing side is diffed as empty
/// text. The patience diff falls back
/// to Myers where no line is unique; a region needing more than 1024 edits there, or any region
/// once a fixed work budget is spent, is shown as replaced as a whole, so the work stays bounded.
///
/// # Errors
///
/// Returns [`LineDiffError::UnknownEntry`] when neither Archive has the Entry, and another
/// [`LineDiffError`] when a side cannot be read or its `CryXml` is corrupt.
pub fn line_diff<'a>(
    old: &'a Archive,
    new: &'a Archive,
    path: &str,
    context: usize,
) -> Result<LineDiff<'a>, LineDiffError> {
    diff(old, new, path, context, None)
}

/// Returns the line diff of the Entry at `path` like [`line_diff`], stopping once `cancel` is
/// set: while a side is read or converted and between the steps.
///
/// # Errors
///
/// Returns [`LineDiffError::Cancelled`] once `cancel` is set, and otherwise the errors of
/// [`line_diff`].
pub fn line_diff_cancellable<'a>(
    old: &'a Archive,
    new: &'a Archive,
    path: &str,
    context: usize,
    cancel: &CancelToken,
) -> Result<LineDiff<'a>, LineDiffError> {
    diff(old, new, path, context, Some(cancel))
}

fn diff<'a>(
    old: &'a Archive,
    new: &'a Archive,
    path: &str,
    context: usize,
    cancel: Option<&CancelToken>,
) -> Result<LineDiff<'a>, LineDiffError> {
    let check = || match cancel {
        Some(token) if token.is_cancelled() => Err(LineDiffError::Cancelled),
        _ => Ok(()),
    };
    check()?;
    let (old_entry, new_entry) = rayon::join(|| find(old, path), || find(new, path));
    if old_entry.is_none() && new_entry.is_none() {
        return Err(LineDiffError::UnknownEntry {
            path: path.to_owned(),
        });
    }
    let (old_side, new_side) = rayon::join(
        || {
            old_entry
                .map(|entry| decode(old, entry, cancel))
                .transpose()
        },
        || {
            new_entry
                .map(|entry| decode(new, entry, cancel))
                .transpose()
        },
    );
    let (old_side, new_side) = (old_side?, new_side?);
    check()?;
    let old_input = LineInput {
        archive: old,
        entry: old_entry,
        outcome: old_side.as_ref().map(Decoded::outcome),
    };
    let new_input = LineInput {
        archive: new,
        entry: new_entry,
        outcome: new_side.as_ref().map(Decoded::outcome),
    };
    let body = match (old_side, new_side) {
        (Some(Decoded::TooLarge(_)), _) | (_, Some(Decoded::TooLarge(_))) => {
            DiffBody::NotText(NotText::TooLarge)
        }
        (Some(Decoded::Binary), _) | (_, Some(Decoded::Binary)) => {
            DiffBody::NotText(NotText::Binary)
        }
        (old_side, new_side) => {
            DiffBody::Text(TextDiff::new(text_of(old_side), text_of(new_side), context))
        }
    };
    Ok(LineDiff {
        old: old_input,
        new: new_input,
        body,
    })
}

fn text_of(side: Option<Decoded>) -> Vec<u8> {
    match side {
        Some(Decoded::Text(text, _)) => text,
        _ => Vec::new(),
    }
}

fn find<'a>(archive: &'a Archive, path: &str) -> Option<&'a Entry> {
    archive
        .entries()
        .par_iter()
        .find_first(|entry| eq_normalised(entry.path(), path))
}
