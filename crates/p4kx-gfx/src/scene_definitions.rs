// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::assets::Assets;
use crate::morph::MorphShape;
use crate::shape::Shape;
use crate::text::{EditText, StaticText};
use crate::timeline::Sprite;

const TAG_DEFINE_BITS: u16 = 6;
const TAG_DEFINE_FONT: u16 = 10;

/// A definition a placement may name.
#[derive(Clone, Copy)]
pub(super) enum Definition<'b> {
    Shape(usize, &'b Shape),
    Morph(usize, &'b MorphShape),
    Sprite(usize, &'b Sprite),
    StaticText(usize, &'b StaticText),
    EditText(usize, &'b EditText),
    Other(u16),
}

impl Definition<'_> {
    pub(super) const fn origin(self) -> Option<super::SceneDefinitionOrigin> {
        use super::SceneDefinitionOrigin as Origin;
        match self {
            Self::Shape(ordinal, _) => Some(Origin::Shape(ordinal)),
            Self::Morph(ordinal, _) => Some(Origin::Morph(ordinal)),
            Self::StaticText(ordinal, _) => Some(Origin::StaticText(ordinal)),
            Self::EditText(ordinal, _) => Some(Origin::EditText(ordinal)),
            Self::Other(_) | Self::Sprite(_, _) => None,
        }
    }
}

/// Indexes the definitions of a movie by ID; the first definition of an ID wins.
pub(super) fn definitions<'b>(assets: &'b Assets<'_>) -> HashMap<u16, Definition<'b>> {
    let mut definitions = HashMap::new();
    for other in &assets.other_definitions {
        definitions
            .entry(other.id)
            .or_insert(Definition::Other(other.code));
    }
    for (ordinal, text) in assets.static_texts.iter().enumerate() {
        definitions
            .entry(text.id)
            .or_insert(Definition::StaticText(ordinal, text));
    }
    for (ordinal, text) in assets.edit_texts.iter().enumerate() {
        definitions
            .entry(text.id)
            .or_insert(Definition::EditText(ordinal, text));
    }
    for font in &assets.fonts {
        definitions
            .entry(font.id)
            .or_insert(Definition::Other(TAG_DEFINE_FONT));
    }
    for bitmap in &assets.bitmaps {
        definitions
            .entry(bitmap.id)
            .or_insert(Definition::Other(TAG_DEFINE_BITS));
    }
    for (ordinal, sprite) in assets.sprites.iter().enumerate() {
        definitions
            .entry(sprite.id)
            .or_insert(Definition::Sprite(ordinal, sprite));
    }
    for (ordinal, morph) in assets.morph_shapes.iter().enumerate() {
        definitions
            .entry(morph.id)
            .or_insert(Definition::Morph(ordinal, morph));
    }
    for (ordinal, shape) in assets.shapes.iter().enumerate() {
        definitions
            .entry(shape.id)
            .or_insert(Definition::Shape(ordinal, shape));
    }
    definitions
}
