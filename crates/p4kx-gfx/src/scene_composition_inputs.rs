// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use super::Context;
use crate::Scene;

/// A normal composed frame with source coordinates for its items and masks.
#[derive(Debug)]
#[non_exhaustive]
pub struct ObservedScene {
    /// The scene emitted by the ordinary composer.
    pub scene: Scene,
    /// The source references retained by the actual composition.
    pub origins: SceneOrigins,
}

/// The retained references from a composed frame into the observed asset walk.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct SceneOrigins {
    /// Placement contexts shared by the items and masks that use them.
    pub placements: Vec<ScenePlacementOrigin>,
    /// Draw references aligned with the scene's item order.
    pub items: Vec<SceneDrawOrigin>,
    /// Draw references aligned with the scene's mask order.
    pub masks: Vec<SceneMaskOrigin>,
    /// Actual glyph outlines indexed by the composed shape they supplied.
    pub glyphs: BTreeMap<usize, SceneGlyphOrigin>,
    /// Font inputs measured by each editable text definition, in character order.
    pub text_layouts: BTreeMap<usize, Vec<Option<SceneTextMeasureOrigin>>>,
}

/// A physical first-frame placement and the sprite context that instantiated it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ScenePlacementOrigin {
    /// Enclosing placement context in the same table.
    pub parent: Option<usize>,
    /// The sprite's asset ordinal, or the main timeline when absent.
    pub timeline: Option<usize>,
    /// Depth in the first frame of that timeline.
    pub depth: u16,
}

/// Physical definition ordinal in the corresponding accepted asset list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SceneDefinitionOrigin {
    /// A shape definition.
    Shape(usize),
    /// A morph shape definition.
    Morph(usize),
    /// A static text definition.
    StaticText(usize),
    /// An editable text definition.
    EditText(usize),
}

/// Sources used by one placed drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SceneDrawOrigin {
    /// Index of the context that placed this drawing.
    pub placement: Option<usize>,
    /// The actual selected shape or text definition.
    pub definition: Option<SceneDefinitionOrigin>,
    /// The shape in the composed scene, including glyph shapes.
    pub shape: usize,
    /// Whether a successful localisation lookup supplied the drawn text.
    pub localised: bool,
    /// Exclusive measured-character prefix used to lay out this glyph.
    pub layout_prefix: Option<usize>,
}

/// A composed mask and the source references of the drawings that form it.
#[derive(Debug)]
#[non_exhaustive]
pub struct SceneMaskOrigin {
    /// The placement context that established the mask and its clipping depth.
    pub placement: Option<usize>,
    /// Drawings in the same order as the scene's mask shapes.
    pub draws: Vec<SceneDrawOrigin>,
}

/// The selected glyph outline in the actual font array supplied to the composer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SceneGlyphOrigin {
    /// Ordinal in the movie fonts followed by the supplied fonts.
    pub font: usize,
    /// Glyph ordinal within the selected font.
    pub glyph: usize,
}

/// The font and glyph queried to measure one editable-text character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SceneTextMeasureOrigin {
    /// Ordinal in the actual supplied font array.
    pub font: usize,
    /// Matched glyph ordinal when the font supplies a code.
    pub glyph: Option<usize>,
    /// Whether layout consumed the glyph's stored advance.
    pub used_advance: bool,
}

impl SceneOrigins {
    pub(crate) fn text_layout(
        &mut self,
        definition: usize,
        measured: impl Iterator<Item = Option<SceneTextMeasureOrigin>>,
    ) {
        self.text_layouts
            .entry(definition)
            .or_insert_with(|| measured.collect());
    }

    pub(super) fn place(
        &mut self,
        parent: Option<usize>,
        timeline: Option<usize>,
        depth: u16,
    ) -> usize {
        let index = self.placements.len();
        self.placements.push(ScenePlacementOrigin {
            parent,
            timeline,
            depth,
        });
        index
    }

    pub(super) fn item(&mut self, context: Context, shape: usize) {
        self.items.push(SceneDrawOrigin::of(context, shape));
    }

    pub(super) fn mask(&mut self, placement: Option<usize>) {
        self.masks.push(SceneMaskOrigin {
            placement,
            draws: Vec::new(),
        });
    }

    pub(super) fn mask_item(&mut self, mask: usize, context: Context, shape: usize) {
        if let Some(mask) = self.masks.get_mut(mask) {
            mask.draws.push(SceneDrawOrigin::of(context, shape));
        }
    }

    pub(crate) fn glyph(&mut self, shape: usize, (font, glyph): (usize, usize)) {
        self.glyphs.insert(shape, SceneGlyphOrigin { font, glyph });
    }
}

impl SceneDrawOrigin {
    const fn of(context: Context, shape: usize) -> Self {
        Self {
            placement: context.placement,
            definition: context.declaration,
            localised: context.localised,
            layout_prefix: context.layout_prefix,
            shape,
        }
    }
}
