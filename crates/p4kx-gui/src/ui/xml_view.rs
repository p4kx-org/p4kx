// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "record_xml_origins.rs"]
mod record_origins;

use std::collections::HashMap;
use std::sync::Arc;

use egui::{
    Color32, Galley, Id, Rect, Response, Role, ScrollArea, Sense, Ui, Vec2, WidgetInfo, pos2, vec2,
};

use crate::shown_line::display;
use crate::theme::desktop::{XML_FONT, XML_GUTTER_PADDING_X, XML_ROW_HEIGHT};
use crate::theme::palette;
use crate::theme::tokens::{Weight, font};
use crate::widgets::scroll_region::{self, Jump};
use crate::{RecordXmlFieldProvenance, Texts, XmlDocument, XmlText};

const MAX_ROWS: usize = 512;

/// The laid-out line numbers and lines shown recently, kept across frames until the document
/// or the scale changes, and where the view stood in the last frame.
#[derive(Clone, Default)]
struct RowCache {
    key: Option<Id>,
    gutter: f32,
    widest: f32,
    rows: HashMap<u32, (Arc<Galley>, Arc<Galley>)>,
}

/// Shows the virtual, numbered lines of `document`; only the visible lines are laid out, once.
///
/// The view takes the keyboard focus with Tab and then scrolls with the arrow keys, Page Up,
/// Page Down, Home and End. With a `jump` it scrolls to that line once.
pub(super) fn show(ui: &mut Ui, texts: &Texts, document: &XmlDocument, jump: Option<Jump<'_>>) {
    show_lines(
        ui,
        texts,
        document,
        jump,
        record_origins::Evidence::Unrequested,
    );
}

pub(super) fn show_record(
    ui: &mut Ui,
    texts: &Texts,
    document: &XmlDocument,
    origins: Option<&[RecordXmlFieldProvenance]>,
) {
    let evidence = origins.map_or(
        record_origins::Evidence::Unknown,
        record_origins::Evidence::Fields,
    );
    show_lines(ui, texts, document, None, evidence);
}

fn show_lines(
    ui: &mut Ui,
    texts: &Texts,
    document: &XmlDocument,
    jump: Option<Jump<'_>>,
    evidence: record_origins::Evidence<'_>,
) {
    let cache_id = ui.scope_id().with("xml-view-row-cache");
    let (mut cache, fresh) = RowCache::load(ui, cache_id, document);
    let total = document.line_count();
    let area = ScrollArea::both().id_salt(cache_id).auto_shrink(false);
    scroll_region::show(
        ui,
        (cache_id, || texts.get(XmlText::View)),
        XML_ROW_HEIGHT,
        area,
        |area, ui| {
            let jumped = Jump::offset(jump, XML_ROW_HEIGHT);
            let area = match jumped {
                Some(y) => area.scroll_offset(vec2(0.0, y)),
                None if fresh => area.scroll_offset(Vec2::ZERO),
                None => area,
            };
            area.show_viewport(ui, |ui, viewport| {
                let origin = ui.max_rect().min;
                let width = viewport.width().max(cache.gutter + cache.widest);
                ui.allocate_rect(
                    Rect::from_min_size(origin, vec2(width, row_top(total))),
                    Sense::hover(),
                );
                let left = origin.x + viewport.min.x;
                let first = row_at(viewport.min.y.floor());
                let last = row_at(viewport.max.y + XML_ROW_HEIGHT - 1.0).min(total);
                for index in first..last {
                    let top = origin.y + row_top(index);
                    let rect = Rect::from_min_size(
                        pos2(left, top),
                        vec2(viewport.width(), XML_ROW_HEIGHT),
                    );
                    let response = paint_row(
                        ui,
                        document,
                        &mut cache,
                        (index, rect),
                        viewport.min.x,
                        evidence.sense(),
                    );
                    evidence.show(response, texts, document, index);
                }
            })
        },
    );
    ui.data_mut(|data| data.insert_temp(cache_id, cache));
}
fn paint_row(
    ui: &Ui,
    document: &XmlDocument,
    cache: &mut RowCache,
    (index, rect): (usize, Rect),
    scroll: f32,
    sense: Sense,
) -> Response {
    let id = u32::try_from(index).unwrap_or(u32::MAX);
    let (number, text) = cache.row(ui, document, id);
    let painter = ui.painter_at(rect);
    let number_right = rect.left() + cache.gutter - XML_GUTTER_PADDING_X;
    painter.galley(
        pos2(number_right - number.size().x, centre(rect, &number)),
        number,
        palette::GRAY_2_TEXT,
    );
    let text_rect = rect.with_min_x(rect.left() + cache.gutter);
    let at = pos2(text_rect.left() - scroll, centre(rect, &text));
    let response = ui.interact(rect, ui.scope_id().with(("xml-line", id)), sense);
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text.text()));
    painter
        .with_clip_rect(text_rect.intersect(painter.clip_rect()))
        .galley(at, text, palette::GRAY_1);
    response
}

fn centre(rect: Rect, galley: &Galley) -> f32 {
    rect.center().y - galley.size().y / 2.0
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a line count fits the precision of a scroll offset"
)]
fn row_top(index: usize) -> f32 {
    index as f32 * XML_ROW_HEIGHT
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a non-negative row position saturates into a line index"
)]
fn row_at(y: f32) -> usize {
    (y / XML_ROW_HEIGHT).floor().max(0.0) as usize
}

fn layout(ui: &Ui, text: String) -> Arc<Galley> {
    ui.fonts_mut(|fonts| {
        fonts.layout_no_wrap(text, font(XML_FONT, Weight::Regular), Color32::PLACEHOLDER)
    })
}

fn layout_line(ui: &Ui, bytes: &[u8]) -> Arc<Galley> {
    let job = display(bytes).job(&font(XML_FONT, Weight::Regular));
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

impl RowCache {
    /// Returns the cache of `document` and whether it was laid out anew.
    fn load(ui: &Ui, cache_id: Id, document: &XmlDocument) -> (Self, bool) {
        let key = Id::unique((
            "xml-view",
            document.serial(),
            ui.ctx().pixels_per_point().to_bits(),
        ));
        let mut cache: Self =
            ui.data_mut(|data| std::mem::take(data.get_temp_mut_or_default::<Self>(cache_id)));
        let fresh = cache.key != Some(key);
        if fresh {
            let digits = document.line_count().max(1).ilog10().saturating_add(1);
            let digit = layout(ui, "0".to_owned()).size().x;
            #[expect(
                clippy::cast_precision_loss,
                reason = "a digit count fits an f32 exactly"
            )]
            let numbers = digits as f32 * digit;
            cache = Self {
                key: Some(key),
                gutter: XML_GUTTER_PADDING_X.mul_add(2.0, numbers),
                widest: layout_line(ui, document.line(document.widest())).size().x,
                ..Self::default()
            };
        }
        (cache, fresh)
    }

    fn row(&mut self, ui: &Ui, document: &XmlDocument, id: u32) -> (Arc<Galley>, Arc<Galley>) {
        if let Some((number, text)) = self.rows.get(&id) {
            return (Arc::clone(number), Arc::clone(text));
        }
        if self.rows.len() >= MAX_ROWS {
            self.rows.clear();
        }
        let number = layout(ui, id.saturating_add(1).to_string());
        let text = layout_line(ui, document.line(id as usize));
        self.rows
            .insert(id, (Arc::clone(&number), Arc::clone(&text)));
        (number, text)
    }
}
