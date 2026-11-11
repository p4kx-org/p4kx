// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod context;
mod declarations;
mod index;
mod resolve;

use std::ops::Range;

pub use index::QuerySchema;

use crate::DataType;

/// The most candidates one completion lists; [`Completion::more`] counts the rest.
pub const MAX_COMPLETIONS: usize = 50;

/// What the candidates of a completion name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CompletionKind {
    /// Struct names, after `from` and in `[…]`.
    Type,
    /// Field names, at the start of a path, after `.` and after `->`.
    Field,
}

/// The names that fit where the cursor stands in a Query text, starting with the word typed so
/// far ignoring ASCII case, in name order.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Completion {
    replace: Range<usize>,
    kind: CompletionKind,
    candidates: Vec<Candidate>,
    more: usize,
}

impl Completion {
    /// Returns the bytes of the text a candidate replaces: the word typed so far, empty at the
    /// cursor when none is.
    #[must_use]
    pub fn replace(&self) -> Range<usize> {
        self.replace.clone()
    }

    /// Returns what the candidates name.
    #[must_use]
    pub const fn kind(&self) -> CompletionKind {
        self.kind
    }

    /// Returns the candidates, at most [`MAX_COMPLETIONS`].
    #[must_use]
    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    /// Returns how many more names fit than are listed.
    #[must_use]
    pub const fn more(&self) -> usize {
        self.more
    }
}

/// One name a completion offers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Candidate {
    name: String,
    insert: String,
    detail: Option<String>,
}

impl Candidate {
    /// Returns the name as DataForge declares it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the name as the Query writes it: in backticks when it is no bare name.
    #[must_use]
    pub fn insert(&self) -> &str {
        &self.insert
    }

    /// Returns the type of a field as Record XML names it, e.g. `Single`, `Int32[]` or
    /// `Reference Manufacturer`; `None` for a struct name.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

/// Where a path stands after its steps.
#[derive(Debug, Clone, Copy)]
enum At {
    Struct(usize),
    Array { data_type: DataType, target: u16 },
    Link(usize),
    Scalar,
}
