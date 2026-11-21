// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Declaration, Mask};

/// Known root fields; unknown declarations remain in the ordered syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RootFieldKind {
    /// Raw version declaration, without inferring a version grammar.
    Version,
    /// Presence of a common-global-flags declaration, without inferred inheritance.
    UsesCommonGlobalFlags,
}

/// Known Property fields; each occurrence remains independently addressable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PropertyFieldKind {
    /// Original flag name.
    Name,
    /// Numeric mask token or uninterpreted expression.
    Mask,
    /// Editor-facing Property label.
    Label,
    /// Authored description.
    Description,
    /// Ordered dependency-set declaration.
    DependencySet,
    /// Ordered dependency-reset declaration.
    DependencyReset,
    /// Hidden marker or other authored Hidden form.
    Hidden,
}

/// A borrowed declaration with access to exact names, values and raw source.
#[derive(Debug, Clone, Copy)]
pub struct Field<'definition> {
    text: &'definition str,
    declaration: &'definition Declaration,
}

impl<'definition> Field<'definition> {
    pub(crate) const fn new(text: &'definition str, declaration: &'definition Declaration) -> Self {
        Self { text, declaration }
    }

    /// Returns the original source-spanned declaration.
    #[must_use]
    pub const fn declaration(self) -> &'definition Declaration {
        self.declaration
    }

    /// Returns the original name token when this construct has one.
    #[must_use]
    pub fn name(self) -> Option<&'definition str> {
        self.declaration.name.and_then(|span| span.text(self.text))
    }

    /// Returns the complete authored construct without normalising its contents.
    #[must_use]
    pub fn raw(self) -> Option<&'definition str> {
        self.declaration.span.text(self.text)
    }

    /// Returns the authored value; markers and blocks have none, explicit empty values remain present.
    #[must_use]
    pub fn value(self) -> Option<&'definition str> {
        self.declaration.value.and_then(|span| span.text(self.text))
    }

    /// Returns the sole lexical atom or quoted token, without decoding escapes or removing quotes.
    #[must_use]
    pub fn literal(self) -> Option<&'definition str> {
        self.declaration
            .literal
            .and_then(|span| span.text(self.text))
    }

    /// Identifies a supported root field name without changing its authored form.
    #[must_use]
    pub fn root_kind(self) -> Option<RootFieldKind> {
        let name = self.name()?;
        if name.eq_ignore_ascii_case("Version") {
            Some(RootFieldKind::Version)
        } else if name.eq_ignore_ascii_case("UsesCommonGlobalFlags") {
            Some(RootFieldKind::UsesCommonGlobalFlags)
        } else {
            None
        }
    }

    /// Identifies a supported Property field name without dropping any other fields.
    #[must_use]
    pub fn property_kind(self) -> Option<PropertyFieldKind> {
        let name = self.name()?;
        let kinds = [
            ("Name", PropertyFieldKind::Name),
            ("Mask", PropertyFieldKind::Mask),
            ("Property", PropertyFieldKind::Label),
            ("Description", PropertyFieldKind::Description),
            ("DependencySet", PropertyFieldKind::DependencySet),
            ("DependencyReset", PropertyFieldKind::DependencyReset),
            ("Hidden", PropertyFieldKind::Hidden),
        ];
        kinds
            .into_iter()
            .find_map(|(known, kind)| name.eq_ignore_ascii_case(known).then_some(kind))
    }

    /// Interprets only explicit hexadecimal masks or the unambiguous zero token.
    #[must_use]
    pub fn mask(self) -> Mask<'definition> {
        Mask::new(self)
    }
}
