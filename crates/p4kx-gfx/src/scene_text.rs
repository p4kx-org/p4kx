// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::rc::Rc;

use crate::font::Font;
use crate::geometry::Rgba;
use crate::import::Import;
use crate::place::Placement;
use crate::scene::{
    Affine, LayerKind, Paint, PathCommand, ScenePath, ScenePoint, SceneShape, ShapeLayer,
};
use crate::scene_compose::{Composer, Context};
use crate::scene_edit_text::Prepared;
use crate::scene_report::SkipReason;
use crate::scene_shape::ShapeBuilder;
use crate::shape::{ShapeRecord, StyleChange};
use crate::style::FillStyle;
use crate::text::StaticText;
use crate::text_fonts::{Fonts, Localise, TextOptions, em, fallback_em};

/// The most characters a scene lays out and draws as glyphs, summed over every text and every
/// placement.
pub const MAX_SCENE_GLYPHS: usize = 1 << 17;

const BLACK: Rgba = Rgba {
    red: 0,
    green: 0,
    blue: 0,
    alpha: u8::MAX,
};
const MAX_REPORTED: usize = 8;
const TOFU_OUTER: [f32; 4] = [100.0, -700.0, 540.0, 0.0];
const TOFU_INSET: f32 = 50.0;

/// The fonts, glyph shapes and prepared texts of one composition.
pub(crate) struct TextState<'b> {
    pub(crate) fonts: Fonts<'b>,
    pub(crate) localise: Option<Localise<'b>>,
    pub(crate) chars: usize,
    pub(crate) prepared: HashMap<u16, Rc<Prepared>>,
    glyph_shapes: HashMap<(usize, usize, u32), usize>,
}

impl<'b> TextState<'b> {
    pub(crate) fn new(
        movie_fonts: &'b [Font],
        imports: &[Import],
        options: &TextOptions<'b>,
    ) -> Self {
        Self {
            fonts: Fonts::new(movie_fonts, options.fonts, imports),
            localise: options.localise,
            chars: 0,
            prepared: HashMap::new(),
            glyph_shapes: HashMap::new(),
        }
    }
}

/// What a text lacked: its font, and the codes or glyph indexes of its glyphs.
#[derive(Debug, Default)]
pub(crate) struct Missing {
    pub(crate) font: Option<u16>,
    codes: Vec<u16>,
    indexes: Vec<u16>,
}

impl Missing {
    pub(crate) fn code(&mut self, code: u16) {
        if self.codes.len() < MAX_REPORTED && !self.codes.contains(&code) {
            self.codes.push(code);
        }
    }

    fn index(&mut self, index: u32) {
        let index = u16::try_from(index).unwrap_or(u16::MAX);
        if self.indexes.len() < MAX_REPORTED && !self.indexes.contains(&index) {
            self.indexes.push(index);
        }
    }
}

fn has_fill(records: &[ShapeRecord]) -> bool {
    records.iter().any(|record| {
        matches!(record, ShapeRecord::StyleChange(change)
            if change.fill_style0.unwrap_or(0) > 0 || change.fill_style1.unwrap_or(0) > 0)
    })
}

/// Converts the outline `records` of a glyph of font `font` to a shape filled with `color`; an
/// outline that names no fill is filled on its right.
pub(crate) fn filled_glyph(
    font: u16,
    color: Rgba,
    records: &[ShapeRecord],
) -> (SceneShape, Vec<SkipReason>) {
    let fills = [FillStyle::Solid { color }];
    if has_fill(records) {
        return ShapeBuilder::convert(font, &fills, &[], records);
    }
    let mut filled = Vec::with_capacity(records.len() + 1);
    filled.push(ShapeRecord::StyleChange(Box::new(StyleChange {
        move_to: None,
        fill_style0: None,
        fill_style1: Some(1),
        line_style: None,
        new_styles: None,
    })));
    filled.extend_from_slice(records);
    ShapeBuilder::convert(font, &fills, &[], &filled)
}

pub(crate) fn tofu() -> ScenePath {
    let [left, top, right, bottom] = TOFU_OUTER;
    let point = |x, y| ScenePoint { x, y };
    let mut commands = Vec::with_capacity(10);
    for inset in [0.0, TOFU_INSET] {
        commands.extend([
            PathCommand::MoveTo(point(left + inset, top + inset)),
            PathCommand::LineTo(point(right - inset, top + inset)),
            PathCommand::LineTo(point(right - inset, bottom - inset)),
            PathCommand::LineTo(point(left + inset, bottom - inset)),
            PathCommand::Close,
        ]);
    }
    ScenePath { commands }
}

impl Composer<'_> {
    /// Charges `count` characters to the glyph limit; the frame ends when it is passed.
    pub(crate) fn charge_chars(
        &mut self,
        placement: &Placement,
        context: Context,
        count: usize,
    ) -> bool {
        self.text.chars = self.text.chars.saturating_add(count);
        if self.text.chars > self.limits.glyphs {
            self.stop(placement, context, SkipReason::TooManyGlyphs);
            return false;
        }
        true
    }

    fn insert_shape(&mut self, shape: SceneShape, issues: Vec<SkipReason>) -> usize {
        let index = self.scene.shapes.len();
        self.scene.shapes.push(shape);
        self.issues.push(issues);
        index
    }

    fn charge_records(&mut self, placement: &Placement, context: Context, cost: usize) -> bool {
        self.records = self.records.saturating_add(cost.max(1));
        if self.records > self.limits.records {
            self.stop(placement, context, SkipReason::TooManyRecords);
            return false;
        }
        true
    }

    /// Returns the shape of glyph `glyph` of font `font` filled with `color`, converted once.
    pub(crate) fn glyph_shape(
        &mut self,
        placement: &Placement,
        context: Context,
        (font, glyph): (usize, usize),
        color: Rgba,
    ) -> Option<usize> {
        let key = (
            font,
            glyph,
            u32::from_be_bytes([color.red, color.green, color.blue, color.alpha]),
        );
        if let Some(&index) = self.text.glyph_shapes.get(&key) {
            return Some(index);
        }
        let font = self.text.fonts.get(font)?;
        let records = &font.glyphs.get(glyph)?.records;
        if !self.charge_records(placement, context, records.len() + 1) {
            return None;
        }
        let (shape, issues) = filled_glyph(font.id, color, records);
        let index = self.insert_shape(shape, issues);
        self.text.glyph_shapes.insert(key, index);
        if let Some(origins) = &mut self.origins {
            origins.glyph(index, (key.0, key.1));
        }
        Some(index)
    }

    /// Returns the shape of the empty box drawn for a missing glyph, in `color`.
    pub(crate) fn tofu_shape(&mut self, color: Rgba) -> usize {
        let key = (
            usize::MAX,
            0,
            u32::from_be_bytes([color.red, color.green, color.blue, color.alpha]),
        );
        if let Some(&index) = self.text.glyph_shapes.get(&key) {
            return index;
        }
        let shape = SceneShape {
            id: u16::MAX,
            layers: vec![ShapeLayer {
                path: tofu(),
                paint: Paint::Solid(color),
                kind: LayerKind::Fill,
            }],
        };
        let index = self.insert_shape(shape, Vec::new());
        self.text.glyph_shapes.insert(key, index);
        index
    }

    /// Draws `shape` with its origin at `at` twips, scaled by `scale`.
    pub(crate) fn draw_glyph(
        &mut self,
        placement: &Placement,
        context: Context,
        inner: Context,
        shape: usize,
        (at, scale): ((f32, f32), f32),
    ) {
        if self
            .scene
            .shapes
            .get(shape)
            .is_none_or(|shape| shape.layers.is_empty())
        {
            return;
        }
        let local = Affine {
            a: scale,
            b: 0.0,
            c: 0.0,
            d: scale,
            tx: at.0,
            ty: at.1,
        };
        let moved = inner.with_transform(inner.transform.then(&local));
        self.draw(placement, context, moved, shape);
    }

    /// Lists what a text lacked.
    pub(crate) fn report_missing(
        &mut self,
        placement: &Placement,
        context: Context,
        missing: Missing,
    ) {
        if let Some(id) = missing.font {
            self.skip(placement, context, SkipReason::MissingFont { id });
        }
        for code in missing.codes {
            self.skip(placement, context, SkipReason::MissingGlyph { code });
        }
        for index in missing.indexes {
            self.skip(placement, context, SkipReason::MissingGlyphIndex { index });
        }
    }

    /// Draws a `DefineText` or `DefineText2` from its glyph entries.
    pub(crate) fn static_text(
        &mut self,
        placement: &Placement,
        context: Context,
        inner: Context,
        text: &StaticText,
    ) {
        let base = inner.with_transform(inner.transform.then(&text.matrix.into()));
        let mut missing = Missing::default();
        let (mut font_id, mut color) = (None, BLACK);
        let (mut x, mut y, mut height) = (0.0_f32, 0.0_f32, 0.0_f32);
        for record in &text.records {
            font_id = record.font_id.or(font_id);
            color = record.color.unwrap_or(color);
            x = record.x_offset.map_or(x, f32::from);
            y = record.y_offset.map_or(y, f32::from);
            height = record.height.map_or(height, f32::from);
            let font = font_id
                .and_then(|id| self.text.fonts.by_id(id))
                .filter(|&at| self.text.fonts.usable(at));
            if font.is_none() && !record.glyphs.is_empty() {
                missing.font = Some(font_id.unwrap_or(u16::MAX));
            }
            for entry in &record.glyphs {
                if self.stopped || !self.charge_chars(placement, context, 1) {
                    return;
                }
                let found = font.and_then(|at| {
                    let font = self.text.fonts.get(at)?;
                    let glyph = usize::try_from(entry.index).ok()?;
                    font.glyphs.get(glyph)?;
                    Some((at, glyph, height / em(font)))
                });
                let (shape, scale) = if let Some((at, glyph, scale)) = found {
                    (
                        self.glyph_shape(placement, context, (at, glyph), color),
                        scale,
                    )
                } else {
                    if font.is_some() {
                        missing.index(entry.index);
                    }
                    (Some(self.tofu_shape(color)), height / fallback_em())
                };
                if let Some(shape) = shape {
                    self.draw_glyph(placement, context, base, shape, ((x, y), scale));
                }
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "advances in twips fit an f32 closely enough"
                )]
                let advance = entry.advance as f32;
                x += advance;
            }
        }
        self.report_missing(placement, context, missing);
    }
}
