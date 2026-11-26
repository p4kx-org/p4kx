// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod incoming;
mod lines;
mod navigation;
mod provenance;
mod worker;

use std::ops::Range;

use p4kx_extraction::{Guid, SourceEvidenceError};

pub(crate) use incoming::IncomingList;
pub use incoming::{IncomingPreview, IncomingStatus, IncomingView, MAX_INCOMING_LINES};
pub(crate) use navigation::Navigation;
pub use provenance::{
    RecordFieldProvenance, RecordHeaderProvenance, RecordPreviewProvenance,
    RecordReferenceProvenance,
};

use crate::{RecordId, TextLanguage, XmlDocument};

/// A Reference as the Record preview shows it: the target's GUID, record path and type, where
/// inside the target it points, and the Record it selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordLink {
    guid: Guid,
    record_path: Box<str>,
    type_name: Box<str>,
    at: Option<Box<str>>,
    tag: Option<Box<str>>,
    target: Option<RecordId>,
}

impl RecordLink {
    /// Returns the GUID of the target Record.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Returns the record path of the target Record.
    #[must_use]
    pub fn record_path(&self) -> &str {
        &self.record_path
    }

    /// Returns the type of the target: its struct, or that of the instance it points at.
    #[must_use]
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns where inside the target Record it points, e.g. `nodes[1]`, or `None` for the
    /// Record itself.
    #[must_use]
    pub fn at(&self) -> Option<&str> {
        self.at.as_deref()
    }

    /// Returns the tag path of the target when it is a Tag Record of the tag tree, e.g.
    /// `Weapon.Gun`.
    #[must_use]
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    /// Returns the Record following the link selects, or `None` when the target does not exist.
    #[must_use]
    pub const fn target(&self) -> Option<RecordId> {
        self.target
    }
}

/// The text a Record preview shows next to a localisation key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyText {
    /// None: no language is chosen, or the key is empty or a placeholder.
    None,
    /// The key's text in the chosen language, on one line: line feeds, control characters and
    /// bidirectional formatting characters as escapes, cut after 1,024 characters.
    Text {
        /// The text as shown.
        text: Box<str>,
        /// The byte ranges of `text` that are escapes.
        escapes: Box<[Range<usize>]>,
    },
    /// The chosen language has no text for the key.
    Missing,
}

/// The value of one line of the Record preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineValue {
    /// A scalar in the text form of Record XML, e.g. `1.5`, `true` or an enum option.
    Scalar(Box<str>),
    /// A localisation key, e.g. `@vehicle_Name`, with its text in the chosen language.
    Key {
        /// The key as the Record stores it, with its leading `@`.
        key: Box<str>,
        /// The text next to it.
        text: KeyText,
    },
    /// An owned struct by its type; its fields follow one level deeper.
    Struct(Box<str>),
    /// An array by its number of elements; the elements follow one level deeper.
    Array(usize),
    /// A null pointer or a null Reference.
    Null,
    /// A Reference to a Record.
    Reference(RecordLink),
    /// A Reference whose target does not exist, with the reason.
    Dangling {
        /// The GUID of the missing Record.
        guid: Guid,
        /// Why the value cannot be read.
        reason: Box<str>,
    },
    /// A value that cannot be read, with the reason.
    Error(Box<str>),
}

/// One field or array element of the Record preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLine {
    depth: u16,
    name: Box<str>,
    value: LineValue,
}

impl FieldLine {
    /// Returns how deep the line sits; the Record's own fields have depth 0.
    #[must_use]
    pub const fn depth(&self) -> u16 {
        self.depth
    }

    /// Returns the field name, or the position of an array element, e.g. `[2]`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the value.
    #[must_use]
    pub const fn value(&self) -> &LineValue {
        &self.value
    }
}

/// One Record read for the preview: its name, type, GUID and record path, every field with the
/// owned children nested below their field, and its Record XML.
#[derive(Debug)]
pub struct RecordView {
    name: Box<str>,
    type_name: Box<str>,
    guid: Guid,
    record_path: Box<str>,
    lines: Vec<FieldLine>,
    errors: usize,
    xml: XmlDocument,
    xml_error: Option<Box<str>>,
    language: Option<TextLanguage>,
    texts_failure: Option<(TextLanguage, Box<str>)>,
    provenance: Option<Result<Box<RecordPreviewProvenance>, SourceEvidenceError>>,
}

impl RecordView {
    /// Returns the Record name, e.g. `EntityClassDefinition.AEGS_Avenger`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the name of the Record's struct.
    #[must_use]
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns the GUID.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Returns the record path.
    #[must_use]
    pub fn record_path(&self) -> &str {
        &self.record_path
    }

    /// Returns the fields and array elements depth first, in declaration and stored order.
    #[must_use]
    pub fn lines(&self) -> &[FieldLine] {
        &self.lines
    }

    /// Returns the original physical contributions to the structured preview's fields.
    ///
    /// An evidence error is separate from whether the Record values could be read.
    #[must_use]
    pub fn provenance(&self) -> Option<Result<&RecordPreviewProvenance, &SourceEvidenceError>> {
        self.provenance.as_ref().map(Result::as_deref)
    }

    /// Returns the number of values that cannot be read.
    #[must_use]
    pub const fn error_count(&self) -> usize {
        self.errors
    }

    /// Returns the Record as Record XML.
    #[must_use]
    pub const fn xml(&self) -> &XmlDocument {
        &self.xml
    }

    /// Returns why the Record XML could not be written, or None when it was.
    #[must_use]
    pub fn xml_error(&self) -> Option<&str> {
        self.xml_error.as_deref()
    }

    /// Returns the language the localisation keys were resolved in, or `None` when the view
    /// shows the keys alone.
    #[must_use]
    pub const fn language(&self) -> Option<TextLanguage> {
        self.language
    }

    /// Returns the language whose texts could not be loaded and why; the view then shows the
    /// keys alone.
    #[must_use]
    pub fn texts_failure(&self) -> Option<(TextLanguage, &str)> {
        self.texts_failure
            .as_ref()
            .map(|(language, reason)| (*language, &**reason))
    }
}

/// Where the preview of the selected Record stands.
#[derive(Debug, Clone, Copy)]
pub enum RecordPreviewStatus<'a> {
    /// The Record is read off the UI thread.
    Loading,
    /// The Record was read.
    Ready(&'a RecordView),
    /// The Record cannot be read, for the reason given, or reading stopped without a result.
    Failed(Option<&'a str>),
    /// The Archive changed on disk, so the Record was not read.
    ArchiveChanged,
}

#[derive(Debug)]
enum State {
    Loading,
    Ready(Box<RecordView>),
    Failed(Option<String>),
    ArchiveChanged,
}

/// The preview of one Record, read on the preview thread of the Archive.
#[derive(Debug)]
pub struct RecordPreview {
    record: RecordId,
    generation: u64,
    language: Option<TextLanguage>,
    pending: Option<u64>,
    state: State,
}

impl RecordPreview {
    /// Returns the language the preview was asked to resolve localisation keys in; the view
    /// shown may still be the one of the language before while the new one is read.
    #[must_use]
    pub const fn language(&self) -> Option<TextLanguage> {
        self.language
    }

    /// Returns the previewed Record.
    #[must_use]
    pub const fn record(&self) -> RecordId {
        self.record
    }

    /// Returns a number no other Record preview of this process has, for caches.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns where the preview stands.
    #[must_use]
    pub fn status(&self) -> RecordPreviewStatus<'_> {
        match &self.state {
            State::Loading => RecordPreviewStatus::Loading,
            State::Ready(view) => RecordPreviewStatus::Ready(view),
            State::Failed(reason) => RecordPreviewStatus::Failed(reason.as_deref()),
            State::ArchiveChanged => RecordPreviewStatus::ArchiveChanged,
        }
    }

    /// Returns the Record read, or `None` while it is read or when it cannot be.
    #[must_use]
    pub fn view(&self) -> Option<&RecordView> {
        match &self.state {
            State::Ready(view) => Some(view),
            _ => None,
        }
    }
}
