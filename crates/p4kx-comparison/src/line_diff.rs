// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use p4kx_cryxml::Document;
use rayon::prelude::*;

use crate::cryxml_lines::{self, CryXmlLines};
use crate::decode::{DecodeOutcome, Decoded, Encoding, LoadedEntry, load};
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
    cryxml_lines: [CryXmlLines; 2],
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

    pub(crate) const fn cryxml_lines(&self) -> &[CryXmlLines; 2] {
        &self.cryxml_lines
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
    let old_input = LineInput {
        archive: old,
        entry: old_entry,
        outcome: None,
    };
    let new_input = LineInput {
        archive: new,
        entry: new_entry,
        outcome: None,
    };
    let mut loaded = load_sides([old_input, new_input], cancel)?;
    let [old_loaded, new_loaded] = &mut loaded;
    let (old_side, new_side) = rayon::join(
        || decode_side(old_loaded, cancel),
        || decode_side(new_loaded, cancel),
    );
    let (old_side, new_side) = (old_side?, new_side?);
    check()?;
    let old = LineInput {
        outcome: old_side.as_ref().map(Decoded::outcome),
        ..old_input
    };
    let new = LineInput {
        outcome: new_side.as_ref().map(Decoded::outcome),
        ..new_input
    };
    let (body, cryxml_lines) = diff_text(old_side, new_side, context, cancel)?;
    Ok(LineDiff {
        old,
        new,
        body,
        cryxml_lines,
    })
}

fn load_sides<'archive>(
    inputs: [LineInput<'archive>; 2],
    cancel: Option<&CancelToken>,
) -> Result<[Option<LoadedEntry<'archive>>; 2], LineDiffError> {
    let [old, new] = inputs;
    let load_side = |side: LineInput<'archive>| {
        side.entry
            .map(|entry| load(side.archive, entry, cancel))
            .transpose()
    };
    let (old, new) = rayon::join(|| load_side(old), || load_side(new));
    let mut old = old?;
    let new = match new {
        Ok(new) => new,
        Err(error) => {
            drop(decode_side(&mut old, cancel)?);
            return Err(error);
        }
    };
    Ok([old, new])
}

fn decode_side<'raw>(
    loaded: &'raw mut Option<LoadedEntry<'_>>,
    cancel: Option<&CancelToken>,
) -> Result<Option<Decoded<'raw>>, LineDiffError> {
    loaded.as_mut().map(|side| side.decode(cancel)).transpose()
}

fn diff_text(
    old: Option<Decoded<'_>>,
    new: Option<Decoded<'_>>,
    context: usize,
    cancel: Option<&CancelToken>,
) -> Result<(DiffBody, [CryXmlLines; 2]), LineDiffError> {
    let non_text = match (&old, &new) {
        (Some(Decoded::TooLarge(_)), _) | (_, Some(Decoded::TooLarge(_))) => {
            Some(NotText::TooLarge)
        }
        (Some(Decoded::Binary), _) | (_, Some(Decoded::Binary)) => Some(NotText::Binary),
        _ => None,
    };
    if let Some(reason) = non_text {
        return Ok((DiffBody::NotText(reason), Default::default()));
    }
    let (old_bytes, old_xml) = text_of(old);
    let (new_bytes, new_xml) = text_of(new);
    let text = TextDiff::new(old_bytes, new_bytes, context);
    let cryxml_lines =
        cryxml_lines::capture(&text, [old_xml.as_deref(), new_xml.as_deref()], cancel)?;
    Ok((DiffBody::Text(text), cryxml_lines))
}

fn text_of(side: Option<Decoded<'_>>) -> (Vec<u8>, Option<Box<Document<'_>>>) {
    match side {
        Some(Decoded::Text(text)) => (text.bytes, text.cryxml),
        _ => (Vec::new(), None),
    }
}

fn find<'a>(archive: &'a Archive, path: &str) -> Option<&'a Entry> {
    archive
        .entries()
        .par_iter()
        .find_first(|entry| eq_normalised(entry.path(), path))
}
