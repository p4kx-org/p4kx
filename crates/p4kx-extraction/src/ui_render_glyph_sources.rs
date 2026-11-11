// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::io;

use p4kx_gfx::{AssetOrigins, SceneDefinitionOrigin, SceneDrawOrigin, SceneOrigins};

use crate::ui_render::ComposedFrame;
use crate::{FieldOrigin, SourceUses, UiMovieEvidence};

/// One font glyph outline admitted by a drawing operation.
#[derive(Debug)]
#[non_exhaustive]
pub struct UiRenderedGlyph {
    /// Ordinal in the movie fonts followed by its resolved imported fonts.
    pub font: usize,
    /// Ordinal within the selected font.
    pub glyph: usize,
    /// The original glyph outline and its actual import selectors.
    pub origin: FieldOrigin,
}

pub(crate) struct TextSources {
    pub(crate) glyphs: Vec<UiRenderedGlyph>,
    pub(crate) localised_definitions: BTreeSet<usize>,
}

#[derive(Default)]
struct SelectedText {
    glyphs: BTreeMap<(usize, usize), bool>,
    localised_definitions: BTreeSet<usize>,
}

pub(crate) fn register(
    uses: &SourceUses,
    movie: &UiMovieEvidence,
    origins: &AssetOrigins,
    frame: &ComposedFrame<'_>,
    items: &[usize],
) -> io::Result<TextSources> {
    let scene = frame.origins.as_ref().ok_or_else(inconsistent)?;
    let mut selected = SelectedText::default();
    let mut masks = BTreeSet::new();
    for &ordinal in items {
        let draw = scene.items.get(ordinal).ok_or_else(inconsistent)?;
        selected.collect(scene, draw);
        let mut mask = frame
            .scene
            .items
            .get(ordinal)
            .ok_or_else(inconsistent)?
            .mask;
        while let Some(ordinal) = mask {
            if !masks.insert(ordinal) {
                break;
            }
            let observed = scene.masks.get(ordinal).ok_or_else(inconsistent)?;
            for draw in &observed.draws {
                selected.collect(scene, draw);
            }
            mask = frame
                .scene
                .masks
                .get(ordinal)
                .ok_or_else(inconsistent)?
                .parent;
        }
    }
    let glyphs = selected
        .glyphs
        .into_iter()
        .map(|((font, glyph), by_code)| {
            let (evidence, complete) = frame
                .fonts
                .glyph(uses, movie, origins, font, glyph, by_code)?;
            Ok(UiRenderedGlyph {
                font,
                glyph,
                origin: uses.origin(&evidence, complete),
            })
        })
        .collect::<io::Result<_>>()?;
    Ok(TextSources {
        glyphs,
        localised_definitions: selected.localised_definitions,
    })
}

impl SelectedText {
    fn collect(&mut self, scene: &SceneOrigins, draw: &SceneDrawOrigin) {
        let edit = match draw.definition {
            Some(SceneDefinitionOrigin::EditText(definition)) => Some(definition),
            _ => None,
        };
        if let Some(glyph) = scene.glyphs.get(&draw.shape) {
            let found = self.glyphs.entry((glyph.font, glyph.glyph)).or_default();
            *found |= edit.is_some();
        }
        if draw.localised
            && let Some(definition) = edit
        {
            self.localised_definitions.insert(definition);
        }
    }
}

fn inconsistent() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "drawn glyph has no scene source",
    )
}
