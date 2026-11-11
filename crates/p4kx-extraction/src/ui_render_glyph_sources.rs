// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::io;

use p4kx_gfx::{AssetOrigins, SceneDefinitionOrigin, SceneDrawOrigin, SceneOrigins};

use super::ui_font_sources::FontUse;
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

/// One font and optional glyph whose metrics supplied text layout.
#[derive(Debug)]
#[non_exhaustive]
pub struct UiRenderedFontMetrics {
    /// Ordinal in the movie fonts followed by its resolved imported fonts.
    pub font: usize,
    /// A glyph selected for measurement, when the font holds the code.
    pub glyph: Option<usize>,
    /// The actual font metrics and their import selectors.
    pub origin: FieldOrigin,
}

pub(crate) struct TextSources {
    pub(crate) glyphs: Vec<UiRenderedGlyph>,
    pub(crate) metrics: Vec<UiRenderedFontMetrics>,
    pub(crate) localised_definitions: BTreeSet<usize>,
}

#[derive(Default)]
struct SelectedText {
    glyphs: BTreeMap<(usize, usize), bool>,
    localised_definitions: BTreeSet<usize>,
    layout_prefixes: BTreeMap<usize, usize>,
}

pub(crate) fn register(
    uses: &SourceUses,
    movie: &UiMovieEvidence,
    origins: &AssetOrigins,
    frame: &ComposedFrame<'_>,
    items: &[usize],
) -> io::Result<TextSources> {
    let scene = frame.origins.as_ref().ok_or_else(inconsistent)?;
    let selected = collect_rendered(frame, scene, items)?;
    let metrics = selected.measurements(scene)?;
    let glyphs = selected
        .glyphs
        .into_iter()
        .map(|((font, glyph), by_code)| {
            let (evidence, complete) = frame.fonts.register(
                uses,
                movie,
                origins,
                font,
                FontUse::Outline { glyph, by_code },
            )?;
            Ok(UiRenderedGlyph {
                font,
                glyph,
                origin: uses.origin(&evidence, complete),
            })
        })
        .collect::<io::Result<_>>()?;
    let metrics = metrics
        .into_iter()
        .map(|((font, glyph), used_advance)| {
            let (evidence, complete) = frame.fonts.register(
                uses,
                movie,
                origins,
                font,
                FontUse::Layout {
                    glyph,
                    used_advance,
                },
            )?;
            Ok(UiRenderedFontMetrics {
                font,
                glyph,
                origin: uses.origin(&evidence, complete),
            })
        })
        .collect::<io::Result<_>>()?;
    Ok(TextSources {
        glyphs,
        metrics,
        localised_definitions: selected.localised_definitions,
    })
}

fn collect_rendered(
    frame: &ComposedFrame<'_>,
    scene: &SceneOrigins,
    items: &[usize],
) -> io::Result<SelectedText> {
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
    Ok(selected)
}

impl SelectedText {
    fn measurements(
        &self,
        scene: &SceneOrigins,
    ) -> io::Result<BTreeMap<(usize, Option<usize>), bool>> {
        let mut result = BTreeMap::new();
        for (&definition, &end) in &self.layout_prefixes {
            let used = scene
                .text_layouts
                .get(&definition)
                .and_then(|layout| layout.get(..end))
                .ok_or_else(inconsistent)?;
            for one in used.iter().flatten() {
                let used_advance = result.entry((one.font, one.glyph)).or_default();
                *used_advance |= one.used_advance;
            }
        }
        Ok(result)
    }

    fn collect(&mut self, scene: &SceneOrigins, draw: &SceneDrawOrigin) {
        let edit = match draw.definition {
            Some(SceneDefinitionOrigin::EditText(definition)) => Some(definition),
            _ => None,
        };
        if let Some(glyph) = scene.glyphs.get(&draw.shape) {
            let found = self.glyphs.entry((glyph.font, glyph.glyph)).or_default();
            *found |= edit.is_some();
        }
        if let Some(definition) = edit {
            if draw.localised {
                self.localised_definitions.insert(definition);
            }
            if let Some(prefix) = draw.layout_prefix {
                let measured = self.layout_prefixes.entry(definition).or_default();
                *measured = (*measured).max(prefix);
            }
        }
    }
}

fn inconsistent() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "drawn glyph has no scene source",
    )
}
