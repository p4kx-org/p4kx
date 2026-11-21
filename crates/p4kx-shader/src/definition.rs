// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Declaration, Field, Form, PropertyFieldKind, RootFieldKind};

/// Supported text encoding without changing original payload bytes or offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Encoding {
    /// UTF-8 without an initial byte-order marker.
    Utf8,
    /// UTF-8 with its original three-byte byte-order marker.
    Utf8Bom,
}

/// Ordered source syntax and presence-aware views over its original bytes.
#[derive(Debug, Clone)]
pub struct Definition<'source> {
    text: &'source str,
    encoding: Encoding,
    declarations: Vec<Declaration>,
}

impl<'source> Definition<'source> {
    pub(crate) const fn new(
        text: &'source str,
        encoding: Encoding,
        declarations: Vec<Declaration>,
    ) -> Self {
        Self {
            text,
            encoding,
            declarations,
        }
    }

    /// Returns the exact original payload, including comments, trivia and any UTF-8 marker.
    #[must_use]
    pub const fn source(&self) -> &'source [u8] {
        self.text.as_bytes()
    }

    /// Returns the validated original source text without escape or whitespace normalisation.
    #[must_use]
    pub const fn text(&self) -> &'source str {
        self.text
    }

    /// Identifies the accepted original encoding.
    #[must_use]
    pub const fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// Returns all top-level declarations, including duplicate and unknown names.
    #[must_use]
    pub fn declarations(&self) -> &[Declaration] {
        &self.declarations
    }

    /// Visits every top-level declaration as a source-backed field.
    pub fn fields(&self) -> impl Iterator<Item = Field<'_>> {
        self.declarations
            .iter()
            .map(move |declaration| Field::new(self.text, declaration))
    }

    /// Visits every declaration of the requested known root field, retaining duplicates.
    pub fn fields_of(&self, kind: RootFieldKind) -> impl Iterator<Item = Field<'_>> {
        self.fields()
            .filter(move |field| field.root_kind() == Some(kind))
    }

    /// Visits top-level braced Property declarations in original source order.
    pub fn properties(&self) -> impl Iterator<Item = Property<'_>> {
        self.fields().filter_map(|field| {
            (field.declaration().form == Form::Block
                && field
                    .name()
                    .is_some_and(|name| name.eq_ignore_ascii_case("Property")))
            .then_some(Property {
                text: self.text,
                declaration: field.declaration(),
            })
        })
    }
}

/// A braced Property whose ordered fields retain their individual authored forms.
#[derive(Debug, Clone, Copy)]
pub struct Property<'definition> {
    text: &'definition str,
    declaration: &'definition Declaration,
}

impl<'definition> Property<'definition> {
    /// Returns the complete source-spanned braced declaration.
    #[must_use]
    pub const fn declaration(self) -> &'definition Declaration {
        self.declaration
    }

    /// Visits every child field, including duplicate names and unknown blocks.
    pub fn fields(self) -> impl Iterator<Item = Field<'definition>> {
        self.declaration
            .children
            .iter()
            .map(move |declaration| Field::new(self.text, declaration))
    }

    /// Visits all occurrences of a known Property field in original order.
    pub fn fields_of(self, kind: PropertyFieldKind) -> impl Iterator<Item = Field<'definition>> {
        self.fields()
            .filter(move |field| field.property_kind() == Some(kind))
    }
}
