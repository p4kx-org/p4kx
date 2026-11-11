// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::rc::Rc;

use crate::geometry::Rgba;
use crate::place::Placement;
use crate::scene_compose::{Composer, Context, SceneTextMeasureOrigin};
use crate::scene_report::SkipReason;
use crate::scene_text::Missing;
use crate::text::EditText;
use crate::text_fonts::{em, fallback_em, vertical_metrics};
use crate::text_html::{Cell, Parsed, Style, html, plain};
use crate::text_layout::{Area, Item, Kind, Placed, layout};
use crate::text_loc::{fill, reference};

const BLACK: Rgba = Rgba {
    red: 0,
    green: 0,
    blue: 0,
    alpha: u8::MAX,
};
const GUTTER: f32 = 40.0;
const DEFAULT_HEIGHT: f32 = 240.0;
const SPACE_ADVANCE: f32 = 0.3;
const TOFU_ADVANCE: f32 = 0.6;
const GLYPH_FALLBACK_ADVANCE: f32 = 0.5;
const PASSWORD_CHAR: char = '*';

/// The characters and styles of a text field after markup, localisation and masking.
#[derive(Debug)]
pub(crate) struct Prepared {
    parsed: Parsed,
    issues: Vec<SkipReason>,
    localised: bool,
}

/// The metrics and the drawing of one character.
#[derive(Debug, Clone, Copy)]
struct Measured {
    item: Item,
    resolved: Resolved,
}

/// The font, glyph and size one laid-out character draws with.
#[derive(Debug, Clone, Copy)]
struct Resolved {
    font: Option<usize>,
    glyph: Option<usize>,
    scale: f32,
    height: f32,
    color: Rgba,
    blank: bool,
    used_advance: bool,
}

impl Composer<'_> {
    fn prepare(
        &mut self,
        placement: &Placement,
        context: Context,
        text: &EditText,
    ) -> Option<Rc<Prepared>> {
        if let Some(prepared) = self.text.prepared.get(&text.id) {
            return Some(Rc::clone(prepared));
        }
        let source = text.text.as_deref().unwrap_or_default();
        let mut parsed = if text.html {
            html(source)
        } else {
            plain(source)
        };
        let mut issues = Vec::new();
        let mut localised = false;
        if parsed.truncated {
            issues.push(SkipReason::TextTruncated);
        }
        if let Some(localise) = self.text.localise {
            let plain_text: String = parsed.cells.iter().map(|cell| cell.ch).collect();
            if let Some(found) = reference(&plain_text) {
                if let Some(template) = localise(found.key) {
                    localised = true;
                    let style = parsed.cells.first().map_or(0, |cell| cell.style);
                    let replaced = plain(&fill(&template, &found.parameters));
                    if replaced.truncated && !issues.contains(&SkipReason::TextTruncated) {
                        issues.push(SkipReason::TextTruncated);
                    }
                    parsed.cells = replaced
                        .cells
                        .into_iter()
                        .map(|cell| Cell { style, ..cell })
                        .collect();
                } else {
                    issues.push(SkipReason::Unlocalised);
                }
            }
        }
        if text.password {
            for cell in &mut parsed.cells {
                if cell.ch != '\n' {
                    cell.ch = PASSWORD_CHAR;
                }
            }
        }
        if !self.charge_chars(placement, context, parsed.cells.len()) {
            return None;
        }
        let prepared = Rc::new(Prepared {
            parsed,
            issues,
            localised,
        });
        self.text.prepared.insert(text.id, Rc::clone(&prepared));
        Some(prepared)
    }

    /// Draws a text field: its text laid out in the bounds with the font, size, colour and
    /// alignment of the field and of its markup.
    pub(crate) fn edit_text(
        &mut self,
        placement: &Placement,
        context: Context,
        inner: Context,
        text: &EditText,
    ) {
        let Some(prepared) = self.prepare(placement, context, text) else {
            return;
        };
        for &issue in &prepared.issues {
            self.skip(placement, context, issue);
        }
        let cells = &prepared.parsed.cells;
        if cells.is_empty() || !self.charge_chars(placement, context, cells.len()) {
            return;
        }
        let mut missing = Missing::default();
        let (items, resolved) = self.items(text, &prepared.parsed, &mut missing);
        if let (Some(origins), Some(definition)) = (&mut self.origins, inner.edit_definition()) {
            origins.text_layout(definition, measured_fonts(&items, &resolved));
        }
        let mut placed = Vec::new();
        layout(&items, &area(text), &mut placed);
        self.draw_placed(
            placement,
            context,
            inner.with_localisation(prepared.localised),
            &resolved,
            &placed,
        );
        self.report_missing(placement, context, missing);
    }

    fn style_fonts(&self, text: &EditText, parsed: &Parsed) -> Vec<Option<usize>> {
        let fonts = &self.text.fonts;
        let default_font = text.font_id.and_then(|id| fonts.by_id(id)).or_else(|| {
            text.font_class
                .as_deref()
                .and_then(|name| fonts.by_name(name))
        });
        parsed
            .styles
            .iter()
            .map(|style| {
                style
                    .face
                    .as_deref()
                    .and_then(|name| fonts.by_name(name))
                    .or(default_font)
                    .filter(|&at| fonts.usable(at))
            })
            .collect()
    }

    fn measure(&self, text: &EditText, ch: char, style: &Style, at: Option<usize>) -> Measured {
        let fonts = &self.text.fonts;
        let font = at.and_then(|at| fonts.get(at));
        let height = style
            .size
            .or_else(|| text.height.map(f32::from))
            .unwrap_or(DEFAULT_HEIGHT)
            .max(1.0);
        let scale = height / font.map_or_else(fallback_em, em);
        let (ascent, descent) = font.map_or((height * 0.8, height * 0.2), |font| {
            vertical_metrics(font, height)
        });
        let glyph = at
            .and_then(|at| fonts.index(at))
            .and_then(|index| index.glyph(ch));
        let kind = match ch {
            '\n' if text.multiline => Kind::Newline,
            '\n' | ' ' | '\u{a0}' => Kind::Space,
            _ => Kind::Glyph,
        };
        let glyph_advance = glyph
            .and_then(|glyph| font?.glyphs.get(glyph)?.advance)
            .map(|advance| f32::from(advance) * scale);
        let advance = match kind {
            Kind::Newline => 0.0,
            Kind::Space => glyph_advance.unwrap_or(height * SPACE_ADVANCE),
            Kind::Glyph if glyph.is_some() => {
                glyph_advance.unwrap_or(height * GLYPH_FALLBACK_ADVANCE)
            }
            Kind::Glyph => height * TOFU_ADVANCE,
        };
        Measured {
            item: Item {
                kind,
                advance: if kind == Kind::Newline {
                    advance
                } else {
                    advance + style.letter_spacing
                },
                ascent,
                descent,
                align: style
                    .align
                    .or_else(|| text.layout.map(|layout| layout.align))
                    .unwrap_or(0),
            },
            resolved: Resolved {
                font: at,
                glyph,
                scale,
                height,
                color: style.color.or(text.color).unwrap_or(BLACK),
                blank: kind != Kind::Glyph,
                used_advance: kind != Kind::Newline && glyph_advance.is_some(),
            },
        }
    }

    fn items(
        &self,
        text: &EditText,
        parsed: &Parsed,
        missing: &mut Missing,
    ) -> (Vec<Item>, Vec<Resolved>) {
        let style_fonts = self.style_fonts(text, parsed);
        let mut items: Vec<Item> = Vec::with_capacity(parsed.cells.len());
        let mut resolved = Vec::with_capacity(parsed.cells.len());
        let mut previous: Option<(usize, char)> = None;
        for cell in &parsed.cells {
            let style_at = usize::from(cell.style);
            let Some(style) = parsed
                .styles
                .get(style_at)
                .or_else(|| parsed.styles.first())
            else {
                continue;
            };
            let at = style_fonts.get(style_at).copied().flatten();
            let measured = self.measure(text, cell.ch, style, at);
            if measured.item.kind == Kind::Glyph {
                if at.is_none() {
                    missing.font = Some(text.font_id.unwrap_or(u16::MAX));
                } else if measured.resolved.glyph.is_none() {
                    missing.code(u16::try_from(u32::from(cell.ch)).unwrap_or(u16::MAX));
                }
                if style.kerning
                    && let Some((before, left)) = previous
                    && Some(before) == at
                    && let Some(last) = items.last_mut()
                    && let Some(index) = self.text.fonts.index(before)
                {
                    let adjustment = f32::from(index.kerning(left, cell.ch));
                    last.advance = adjustment.mul_add(measured.resolved.scale, last.advance);
                }
                previous = at.map(|at| (at, cell.ch));
            } else {
                previous = None;
            }
            items.push(measured.item);
            resolved.push(measured.resolved);
        }
        (items, resolved)
    }
    fn draw_placed(
        &mut self,
        placement: &Placement,
        context: Context,
        inner: Context,
        resolved: &[Resolved],
        placed: &[Placed],
    ) {
        for spot in placed {
            if self.stopped {
                return;
            }
            let Some(&one) = resolved.get(spot.item) else {
                continue;
            };
            let at = (spot.x, spot.baseline);
            let inner = inner.with_layout_prefix(spot.inspected);
            if let (Some(font), Some(glyph)) = (one.font, one.glyph) {
                let shape = self.glyph_shape(placement, context, (font, glyph), one.color);
                if let Some(shape) = shape {
                    self.draw_glyph(placement, context, inner, shape, (at, one.scale));
                }
            } else if !one.blank {
                let shape = self.tofu_shape(one.color);
                let scale = one.height / fallback_em();
                self.draw_glyph(placement, context, inner, shape, (at, scale));
            }
        }
    }
}

fn measured_fonts<'a>(
    items: &'a [Item],
    resolved: &'a [Resolved],
) -> impl Iterator<Item = Option<SceneTextMeasureOrigin>> + 'a {
    let mut has_content = false;
    items.iter().zip(resolved).map(move |(item, one)| {
        let newline = item.kind == Kind::Newline;
        let uses_font = !newline || !has_content;
        has_content = !newline;
        one.font
            .filter(|_| uses_font)
            .map(|font| SceneTextMeasureOrigin {
                font,
                glyph: one.glyph,
                used_advance: one.used_advance,
            })
    })
}

#[expect(
    clippy::cast_precision_loss,
    reason = "bounds in twips fit an f32 closely enough"
)]
fn area(text: &EditText) -> Area {
    let layout = text.layout;
    let left_margin = layout.map_or(0.0, |layout| f32::from(layout.left_margin));
    let right_margin = layout.map_or(0.0, |layout| f32::from(layout.right_margin));
    Area {
        left: text.bounds.x_min as f32 + GUTTER + left_margin,
        width: (text.bounds.width() as f32 - GUTTER.mul_add(2.0, left_margin) - right_margin)
            .max(0.0),
        top: text.bounds.y_min as f32 + GUTTER,
        wrap: text.word_wrap,
        indent: layout.map_or(0.0, |layout| f32::from(layout.indent)),
        leading: layout.map_or(0.0, |layout| f32::from(layout.leading)),
    }
}
