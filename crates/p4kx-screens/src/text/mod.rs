// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod draw;
mod fit;
mod fonts;
mod layout;
#[cfg(test)]
pub(crate) mod probe;
#[cfg(test)]
mod reuse_tests;
mod source;
mod spec;

use std::fmt;

use p4kx_gfx::{Font, Localise};

pub(crate) use draw::Shaped;
pub(crate) use fonts::{FontStyleInfo, FontStyles};
pub use source::EDITOR_LABEL;
pub(crate) use source::{is_editor_label, is_placeholder};
pub(crate) use spec::{Case, Horizontal, TEXT, TEXT_FIELD, TextSpec, Vertical};

use fonts::FontBook;

/// The fonts, the language and the budget the texts of one composition draw with.
#[derive(Default)]
pub(crate) struct Texts<'t> {
    book: Option<FontBook<'t>>,
    localise: Option<Localise<'t>>,
    pub(crate) styles: FontStyles,
    chars: usize,
}

impl<'t> Texts<'t> {
    pub(crate) fn new(fonts: &'t [Font], localise: Option<Localise<'t>>) -> Self {
        Self {
            book: Some(FontBook::new(fonts)),
            localise,
            styles: FontStyles::default(),
            chars: 0,
        }
    }

    pub(crate) const fn is_enabled(&self) -> bool {
        self.book.is_some()
    }
}

impl fmt::Debug for Texts<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Texts")
            .field("fonts", &self.book.is_some())
            .field("localise", &self.localise.is_some())
            .field("chars", &self.chars)
            .finish_non_exhaustive()
    }
}
