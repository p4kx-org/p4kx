// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gfx::{FontFace, MAX_SCENE_GLYPHS, MAX_TEXT_CHARS, MISSING_GLYPH_EM, Rgba, missing_glyph};

use super::fit::{Fit, MIN_SIZE, shape_fitted};
use super::layout::{self, Cell, Shaping};
use super::source::{is_placeholder, resolve};
use super::spec::{Horizontal, TextSpec, Vertical};
use super::{Texts, fonts::FontBook};
use crate::context::Context;
use crate::glyphs::{self, MISSING};
use crate::look::Look;
use crate::paint::{Area, Painter, TWIPS};
use crate::widget::Widget;

const MAX_MISSING_REPORTED: usize = 8;
const MAX_SIZE: f32 = 1024.0;
const FIELD_SIZE_OF_HEIGHT: f32 = 0.6;
const MIN_LINE_OF_SIZE: f32 = 0.1;

pub(crate) type Notes = Vec<String>;

#[derive(Debug)]
pub(crate) struct Prepared {
    font: usize,
    size: f32,
    cells: Vec<Cell>,
    line_spacing: f32,
    horizontal: Horizontal,
    vertical: Vertical,
    wrap: bool,
    height_input: Option<f32>,
    fit: Option<Fit>,
}

impl Prepared {
    /// Returns whether shaping the text again in `area` would give this result: the height is
    /// the one a size taken from the height was counted from, and the width still holds a fitted
    /// text.
    fn holds_for(&self, area: Area) -> bool {
        let height = self
            .height_input
            .is_none_or(|height| height.to_bits() == area.height.to_bits());
        let width = self.fit.as_ref().is_none_or(|fit| {
            fit.width.to_bits() == area.width.to_bits() || (!fit.shrunk && fit.widest <= area.width)
        });
        height && width
    }
}

/// A text shaped once, with what the shaping noted; `prepared` is `None` for a text that is not
/// drawn.
#[derive(Debug)]
pub(crate) struct Shaped {
    pub(crate) prepared: Option<Prepared>,
    notes: Notes,
}

#[cfg(test)]
use super::probe;

#[cfg(test)]
fn may_reuse() -> bool {
    probe::reuse()
}

#[cfg(not(test))]
const fn may_reuse() -> bool {
    true
}

impl Texts<'_> {
    pub(crate) fn shape(&mut self, widget: &Widget, look: &Look<'_>, area: Area) -> Shaped {
        #[cfg(test)]
        probe::shaped();
        let mut notes = Notes::new();
        let prepared = self.prepare(widget, look, area, &mut notes);
        Shaped { prepared, notes }
    }

    fn prepare(
        &mut self,
        widget: &Widget,
        look: &Look<'_>,
        area: Area,
        notes: &mut Notes,
    ) -> Option<Prepared> {
        let spec = widget.text.as_ref()?;
        let book = self.book.as_mut()?;
        let modifiers = &look.style.text;
        let source = modifiers
            .text
            .as_deref()
            .or(look.bound_text.as_deref())
            .unwrap_or(&spec.source)
            .trim();
        if is_placeholder(source) {
            notes.push("text is empty or filled in by the game at run time".to_owned());
            return None;
        }
        let resolved = resolve(self.localise, source, notes);
        let cased = modifiers.case.unwrap_or(spec.case).apply(&resolved);
        if cased.trim().is_empty() {
            notes.push("text is empty or filled in by the game at run time".to_owned());
            return None;
        }
        let font = pick_font(book, widget, look, spec, notes)?;
        let size = font_size(look, spec, area, font.1, notes);
        let limited = cased.chars().take(MAX_TEXT_CHARS).collect::<String>();
        if limited.len() < cased.len() {
            notes.push(format!("text cut to {MAX_TEXT_CHARS} characters"));
        }
        let fit = modifiers
            .auto_size
            .unwrap_or(spec.auto_size)
            .then_some(area.width);
        let how = Shaping {
            size,
            letter_spacing: modifiers
                .letter_spacing
                .unwrap_or(spec.letter_spacing)
                .clamp(-MAX_SIZE, MAX_SIZE)
                * look.pixel,
            kerning: spec.kerning,
        };
        let (cells, size, fit) = shape_fitted(book.face(font.0)?, &limited, how, fit);
        Some(Prepared {
            height_input: (modifiers.font_size.or(spec.font_size).is_none()).then_some(area.height),
            fit,
            font: font.0,
            size,
            cells,
            line_spacing: modifiers
                .line_spacing
                .unwrap_or(spec.line_spacing)
                .clamp(-MAX_SIZE, MAX_SIZE)
                * look.pixel,
            horizontal: modifiers.horizontal.unwrap_or(spec.horizontal),
            vertical: modifiers.vertical.unwrap_or(spec.vertical),
            wrap: modifiers.wrap.unwrap_or(spec.wrap),
        })
    }

    pub(crate) fn measure(&self, prepared: &Prepared, parent: Area) -> Option<[f32; 2]> {
        let (ascent, descent) = self.book.as_ref()?.face(prepared.font)?.ascent_descent();
        let limit = prepared.wrap.then_some(parent.width);
        let lines = layout::lines(&prepared.cells, limit);
        let widest = lines
            .iter()
            .filter_map(|line| prepared.cells.get(line.clone()))
            .map(layout::width)
            .fold(0.0_f32, f32::max);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a text has at most a few thousand lines"
        )]
        let height = lines.len() as f32 * line_height(prepared, ascent, descent);
        Some([widest, height])
    }
}

fn pick_font(
    book: &mut FontBook<'_>,
    widget: &Widget,
    look: &Look<'_>,
    spec: &TextSpec,
    notes: &mut Notes,
) -> Option<(usize, f32)> {
    let info = look.font.as_deref();
    let scale = info.map_or(1.0, |info| info.scale);
    if let Some(at) = info.and_then(|info| book.pick(info)) {
        return Some((at, scale));
    }
    notes.push(match (info, &spec.preset) {
        (Some(info), _) => format!(
            "font style {} has no font in the font movie: default font used",
            info.name
        ),
        (None, Some(preset)) => style_note(
            preset,
            "has no font style from the cascade: default font used",
        ),
        (None, None) => "no font style: default font used".to_owned(),
    });
    let default = book.default_font();
    if default.is_none() {
        notes.push(format!(
            "no usable font in the font movie for {}",
            widget.name
        ));
    }
    default.map(|at| (at, scale))
}

/// Returns the note about the text style `preset`: `what` follows its name.
fn style_note(preset: &str, what: &str) -> String {
    format!("text style {preset} {what}")
}

fn font_size(look: &Look<'_>, spec: &TextSpec, area: Area, scale: f32, notes: &mut Notes) -> f32 {
    let size = look.style.text.font_size.or(spec.font_size).map_or_else(
        || {
            notes.push(spec.preset.as_deref().map_or_else(
                || "no font size: taken from the height of the widget".to_owned(),
                |preset| {
                    style_note(
                        preset,
                        "has no font size in the Archive: taken from the height of the widget",
                    )
                },
            ));
            area.height * FIELD_SIZE_OF_HEIGHT
        },
        |pixels| pixels * look.pixel,
    );
    (size * scale).clamp(MIN_SIZE * look.pixel, MAX_SIZE * look.pixel)
}

fn line_height(prepared: &Prepared, ascent: f32, descent: f32) -> f32 {
    let natural = (ascent + descent) * prepared.size;
    (natural + prepared.line_spacing).max(prepared.size * MIN_LINE_OF_SIZE)
}

struct LineDrawer<'f, 'a> {
    face: &'f FontFace<'a>,
    font: usize,
    colour: Rgba,
    glyph_scale: f32,
    box_scale: f32,
    missing: Vec<char>,
}

impl LineDrawer<'_, '_> {
    fn draw(&mut self, painter: &mut Painter, cells: &[Cell], (start, baseline): (f32, f32)) {
        let mut pen = start;
        for cell in cells {
            if cell.is_visible() {
                self.place(painter, cell, (pen, baseline));
            }
            pen += cell.advance;
        }
    }

    fn place(&mut self, painter: &mut Painter, cell: &Cell, at: (f32, f32)) {
        let colour = self.colour;
        if let Some(glyph) = cell.glyph {
            let face = self.face;
            painter.glyph(
                glyphs::key(self.font, glyph, colour),
                || face.outline(glyph, colour),
                (at, self.glyph_scale),
            );
            return;
        }
        if self.missing.len() < MAX_MISSING_REPORTED && !self.missing.contains(&cell.ch) {
            self.missing.push(cell.ch);
        }
        painter.glyph(
            glyphs::key(self.font, MISSING, colour),
            || Some(missing_glyph(colour)),
            (at, self.box_scale),
        );
    }
}

impl Context<'_> {
    pub(crate) fn text(&mut self, widget: &Widget, look: &Look<'_>, area: Area, alpha: f32) {
        let reused = look.shaped.as_ref().filter(|shaped| {
            may_reuse() && shaped.prepared.as_ref().is_some_and(|p| p.holds_for(area))
        });
        let fresh;
        let shaped = if let Some(shaped) = reused {
            shaped
        } else {
            fresh = self.texts.shape(widget, look, area);
            &fresh
        };
        for reason in &shaped.notes {
            self.report.note(&widget.type_name, reason, &widget.name);
        }
        let prepared = shaped.prepared.as_ref();
        let colour_spec = widget
            .text
            .as_ref()
            .and_then(|spec| look.fill.or(spec.colour.as_ref()));
        if colour_spec.is_none() {
            self.report
                .note(&widget.type_name, "text has no colour", &widget.name);
        }
        let colour = colour_spec.and_then(|spec| self.colour(spec, alpha, widget));
        let (Some(prepared), Some(colour)) = (prepared, colour) else {
            self.report.texts_left_out += 1;
            return;
        };
        if colour.alpha == 0 {
            self.report.note(
                &widget.type_name,
                "text is fully transparent at rest",
                &widget.name,
            );
            self.report.texts_left_out += 1;
            return;
        }
        self.texts.chars = self.texts.chars.saturating_add(prepared.cells.len());
        if self.texts.chars > MAX_SCENE_GLYPHS {
            self.report.truncated = true;
            self.report.texts_left_out += 1;
            return;
        }
        if self.place_text(widget, prepared, area, colour) {
            self.report.texts_drawn += 1;
        } else {
            self.report.texts_left_out += 1;
        }
    }

    fn place_text(
        &mut self,
        widget: &Widget,
        prepared: &Prepared,
        area: Area,
        colour: Rgba,
    ) -> bool {
        let Some(face) = self
            .texts
            .book
            .as_ref()
            .and_then(|book| book.face(prepared.font))
        else {
            return false;
        };
        let (ascent, descent) = face.ascent_descent();
        let height = line_height(prepared, ascent, descent);
        let limit = prepared.wrap.then_some(area.width);
        let lines = layout::lines(&prepared.cells, limit);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a text has at most a few thousand lines"
        )]
        let total = lines.len() as f32 * height;
        let top = area.y
            + match prepared.vertical {
                Vertical::Top => 0.0,
                Vertical::Center => (area.height - total) / 2.0,
                Vertical::Bottom => area.height - total,
            };
        let before = self.painter.len();
        let mut line_drawer = LineDrawer {
            face,
            font: prepared.font,
            colour,
            glyph_scale: prepared.size * TWIPS / face.units_per_em(),
            box_scale: prepared.size * TWIPS / MISSING_GLYPH_EM,
            missing: Vec::new(),
        };
        for (row, line) in lines.iter().enumerate() {
            let Some(cells) = prepared.cells.get(line.clone()) else {
                continue;
            };
            #[expect(
                clippy::cast_precision_loss,
                reason = "a text has at most a few thousand lines"
            )]
            let row_offset = row as f32 * height;
            let baseline = ascent.mul_add(prepared.size, top + row_offset);
            let free = area.width - layout::width(cells);
            let pen = area.x
                + match prepared.horizontal {
                    Horizontal::Left => 0.0,
                    Horizontal::Center => free / 2.0,
                    Horizontal::Right => free,
                };
            line_drawer.draw(&mut self.painter, cells, (pen, baseline));
        }
        self.note_missing(widget, &line_drawer.missing);
        self.painter.len() > before
    }

    fn note_missing(&mut self, widget: &Widget, missing: &[char]) {
        for ch in missing {
            self.report.note(
                &widget.type_name,
                &format!(
                    "character U+{:04X} has no glyph in the font: drawn as an empty box",
                    u32::from(*ch)
                ),
                &widget.name,
            );
        }
    }
}

#[cfg(test)]
mod tests;
