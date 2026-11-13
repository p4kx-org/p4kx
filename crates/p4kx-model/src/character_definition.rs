// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Positioned declarations and bone-relative offsets in a character definition.

use p4kx_cryxml::{
    Format, SourceAttribute, SourceDocument, SourceElement, SourceReadError, SourceReadOptions,
};
use p4kx_foundation::read_text_number;

/// A CDF source with original XML coordinates.
#[derive(Debug)]
pub struct CharacterDefinition<'input> {
    source: SourceDocument<'input>,
}

impl<'input> CharacterDefinition<'input> {
    /// Parses original CDF Entry bytes, including other XML root types.
    ///
    /// # Errors
    /// Returns the XML error for malformed input.
    pub fn read(bytes: &'input [u8]) -> Result<Self, SourceReadError> {
        let source = SourceDocument::parse(bytes, SourceReadOptions::default())?;
        Ok(Self { source })
    }

    /// Returns whether this document declares a character definition.
    #[must_use]
    pub fn is_character_definition(&self) -> bool {
        self.root()
            .name()
            .eq_ignore_ascii_case("CharacterDefinition")
    }

    /// Returns the root declaration and its original coordinate.
    #[must_use]
    pub fn root(&self) -> SourceElement<'_> {
        self.source.root()
    }

    /// Returns the original text or binary encoding.
    #[must_use]
    pub const fn format(&self) -> Format {
        self.source.format()
    }

    /// Returns whether the binary reader repaired string bytes.
    #[must_use]
    pub const fn is_lossy(&self) -> bool {
        self.source.is_lossy()
    }

    /// Visits model declarations with their original root child ordinals.
    pub fn models(&self) -> impl Iterator<Item = (usize, SourceElement<'_>)> {
        self.is_character_definition()
            .then(|| self.root())
            .into_iter()
            .flat_map(SourceElement::elements)
            .enumerate()
            .filter(|(_, child)| child.name().eq_ignore_ascii_case("Model"))
    }

    /// Visits attachments with their original list and child ordinals.
    pub fn attachments(&self) -> impl Iterator<Item = ([usize; 2], SourceElement<'_>)> {
        self.is_character_definition()
            .then(|| self.root())
            .into_iter()
            .flat_map(SourceElement::elements)
            .enumerate()
            .filter(|(_, child)| child.name().eq_ignore_ascii_case("AttachmentList"))
            .flat_map(|(list, parent)| {
                parent
                    .elements()
                    .enumerate()
                    .filter(|(_, child)| child.name().eq_ignore_ascii_case("Attachment"))
                    .map(move |(child, element)| ([list, child], element))
            })
    }
}

/// Returns whether the path has a nonempty stem and the CDF extension.
#[must_use]
pub fn is_definition_path(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(stem, extension)| !stem.is_empty() && extension.eq_ignore_ascii_case("cdf"))
}

/// Returns a named CDF attribute with its original XML coordinate.
#[must_use]
pub fn attribute<'element>(
    element: SourceElement<'element>,
    name: &str,
) -> Option<SourceAttribute<'element>> {
    element
        .attributes()
        .find(|attr| attr.namespace().is_none() && attr.name().eq_ignore_ascii_case(name))
}

/// Decodes the position from the parent bone in metres, defaulting an absent value to zero.
#[must_use]
pub fn relative_position(authored: Option<&str>) -> Option<[f32; 3]> {
    authored.map_or(Some([0.0; 3]), numbers)
}

/// Decodes the bone-relative [w, x, y, z] quaternion, defaulting to identity only when absent.
#[must_use]
pub fn relative_rotation(authored: Option<&str>) -> Option<[f32; 4]> {
    authored.map_or(Some([1.0, 0.0, 0.0, 0.0]), |text| {
        numbers::<4>(text).filter(|value| value.iter().any(|part| *part != 0.0))
    })
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "CDF offsets use single precision and reject nonfinite conversions"
)]
fn numbers<const N: usize>(text: &str) -> Option<[f32; N]> {
    let mut values = [0.0; N];
    let mut parts = text.split(',');
    for value in &mut values {
        let number = read_text_number(parts.next()?).filter(|number| !number.is_partial())?;
        let part = number.value() as f32;
        *value = part.is_finite().then_some(part)?;
    }
    parts.next().is_none().then_some(values)
}
