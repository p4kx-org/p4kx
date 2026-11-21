// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use egui::{Galley, Rect, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use super::chip::{self, Tone};
use super::frames::accent_edge;
use super::rows::chevron;
use super::text::{caps, line, paint_left, plain};
use crate::theme::desktop::{
    ACCENT_BAR, CHEVRON, CHIP_PADDING_Y, FACT_KEY_GAP, FACT_KEY_SIZE, FACT_KEY_TRACKING,
    FACT_VALUE_MIN_CHARS, FACT_VALUE_SIZE, FACTS_GAP, FACTS_PADDING_X, FACTS_PADDING_Y,
    KPI_PADDING_X, KPI_PADDING_Y, KPI_VALUE_SIZE,
};
use crate::theme::palette;
use crate::theme::tokens::{CORNER_SM, CORNER_TILE, RADIUS_SM, SPACE_1, Weight};

/// One fact of a `.facts-bar`: an upper-case key and a value.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fact<'a> {
    pub(crate) key: &'a str,
    pub(crate) value: &'a str,
}

/// Shows a `.facts-bar` with `old` and `new`, an orange chevron between them for the direction.
pub(crate) fn facts_bar(ui: &mut Ui, old: Fact<'_>, new: Fact<'_>) {
    let key = |text: &str| caps(text, FACT_KEY_SIZE, Weight::Bold, FACT_KEY_TRACKING);
    let value = |text: &str| plain(text, FACT_VALUE_SIZE, Weight::Bold);
    let galleys = [
        line(ui, key(old.key), f32::INFINITY),
        line(ui, value(old.value), f32::INFINITY),
        line(ui, key(new.key), f32::INFINITY),
        line(ui, value(new.value), f32::INFINITY),
    ];
    let tallest = galleys
        .iter()
        .map(|galley| galley.size().y)
        .fold(CHEVRON, f32::max);
    let height = 2.0f32.mul_add(FACTS_PADDING_Y, tallest);
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    response.widget_info(|| {
        let label = format!("{} {} {} {}", old.key, old.value, new.key, new.value);
        WidgetInfo::labeled(Role::Label, true, label)
    });
    paint_bar(ui, rect);
    let mut left = rect.left() + FACTS_PADDING_X;
    let [old_key, old_value, new_key, new_value] = galleys;
    for (index, (galley, colour)) in [
        (old_key, palette::GRAY_2_TEXT),
        (old_value, palette::WHITE),
        (new_key, palette::GRAY_2_TEXT),
        (new_value, palette::WHITE),
    ]
    .into_iter()
    .enumerate()
    {
        if index == 2 {
            let centre = pos2(left + CHEVRON / 2.0, rect.center().y);
            chevron(ui.painter(), centre, CHEVRON, false, palette::PRIMARY);
            left += CHEVRON + FACTS_GAP;
        }
        let width = galley.size().x;
        paint_left(ui, rect.with_min_x(left), galley, colour);
        left += width
            + if index % 2 == 0 {
                FACT_KEY_GAP
            } else {
                FACTS_GAP
            };
    }
}

/// Paints the `radius-sm` bar surface of a `.facts-bar` with its hairline under it, which ends
/// where the corners begin.
fn paint_bar(ui: &Ui, rect: Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, CORNER_SM, palette::SURFACE_BAR);
    painter.hline(
        rect.x_range().shrink(RADIUS_SM),
        rect.bottom() - 0.5,
        Stroke::new(1.0, palette::GRAY_3),
    );
}

/// The laid-out facts of a `.facts-bar` without a direction, kept across frames.
pub(crate) struct FactsStrip {
    facts: Vec<(Arc<Galley>, Arc<Galley>)>,
    label: String,
    height: f32,
}

impl FactsStrip {
    /// Lays `facts` out as upper-case keys followed by their values.
    pub(crate) fn lay_out(ui: &Ui, facts: &[Fact<'_>]) -> Self {
        let laid: Vec<(Arc<Galley>, Arc<Galley>)> = facts
            .iter()
            .map(|fact| {
                let key = caps(fact.key, FACT_KEY_SIZE, Weight::Bold, FACT_KEY_TRACKING);
                let value = plain(fact.value, FACT_VALUE_SIZE, Weight::Bold);
                (line(ui, key, f32::INFINITY), line(ui, value, f32::INFINITY))
            })
            .collect();
        let tallest = laid
            .iter()
            .map(|(key, value)| key.size().y.max(value.size().y))
            .fold(0.0, f32::max);
        let label = facts
            .iter()
            .map(|fact| format!("{} {}", fact.key, fact.value))
            .collect::<Vec<_>>()
            .join(" ");
        Self {
            facts: laid,
            label,
            height: 2.0f32.mul_add(FACTS_PADDING_Y, tallest),
        }
    }

    /// Returns the width the facts take from the left edge of the bar to the end of the last.
    pub(crate) fn width(&self) -> f32 {
        let facts: f32 = self
            .facts
            .iter()
            .map(|(key, value)| key.size().x + FACT_KEY_GAP + value.size().x + FACTS_GAP)
            .sum();
        FACTS_PADDING_X + facts
    }

    /// Shows the facts as a `.facts-bar` across the available width and returns its rectangle.
    ///
    /// A value that does not fit is cut with an ellipsis, and the facts after the last that fits
    /// are left out with an ellipsis ending the bar, so nothing is painted outside it.
    pub(crate) fn show(&self, ui: &mut Ui) -> Rect {
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), self.height), Sense::hover());
        response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &self.label));
        paint_bar(ui, rect);
        let limit = rect.right() - FACTS_PADDING_X;
        let mut left = rect.left() + FACTS_PADDING_X;
        for (key, value) in &self.facts {
            let key_width = key.size().x;
            let after_key = left + key_width + FACT_KEY_GAP;
            let room = limit - after_key;
            if room < FACT_VALUE_SIZE * FACT_VALUE_MIN_CHARS {
                let more = line(ui, plain("…", FACT_VALUE_SIZE, Weight::Bold), f32::INFINITY);
                let at = left.min(limit - more.size().x);
                paint_left(ui, rect.with_min_x(at), more, palette::GRAY_2_TEXT);
                break;
            }
            paint_left(
                ui,
                rect.with_min_x(left),
                Arc::clone(key),
                palette::GRAY_2_TEXT,
            );
            let value = if value.size().x > room {
                line(ui, plain(value.text(), FACT_VALUE_SIZE, Weight::Bold), room)
            } else {
                Arc::clone(value)
            };
            let value_width = value.size().x;
            paint_left(ui, rect.with_min_x(after_key), value, palette::WHITE);
            left = after_key + value_width + FACTS_GAP;
        }
        rect
    }
}

/// Shows a `.kpi-total` tile of `width` with the chip `label` in `tone` over the `value`.
pub(crate) fn kpi_tile(ui: &mut Ui, label: &str, value: &str, tone: Tone, width: f32) {
    let label_galley = chip::galley(ui, label);
    let value_galley = line(ui, plain(value, KPI_VALUE_SIZE, Weight::Bold), width);
    let chip_height = 2.0f32.mul_add(CHIP_PADDING_Y, label_galley.size().y);
    let height = 2.0f32.mul_add(
        KPI_PADDING_Y,
        chip_height + value_galley.size().y + SPACE_1 / 2.0,
    );
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, format!("{label} {value}")));
    let painter = ui.painter();
    painter.rect(
        rect,
        CORNER_TILE,
        palette::SURFACE_CARD,
        Stroke::new(1.0, palette::GRAY_3),
        StrokeKind::Inside,
    );
    let bar = Rect::from_min_size(rect.min, vec2(ACCENT_BAR, rect.height()));
    accent_edge(painter, (rect, CORNER_TILE), bar, palette::PRIMARY);
    let inner = rect.shrink2(vec2(KPI_PADDING_X, KPI_PADDING_Y));
    chip::paint(
        ui,
        pos2(inner.left(), inner.top() + chip_height / 2.0),
        label_galley,
        tone,
    );
    ui.painter().galley(
        pos2(inner.left(), inner.top() + chip_height + SPACE_1 / 2.0),
        value_galley,
        palette::WHITE,
    );
}

#[cfg(test)]
mod tests {
    use egui::epaint::Shape;
    use egui_kittest::Harness;

    use super::{FACTS_PADDING_X, Fact, FactsStrip};

    const WIDTH: f32 = 200.0;

    fn texts(shape: &Shape, found: &mut Vec<(String, f32)>) {
        match shape {
            Shape::Text(text) => {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                found.push((text.galley.text().to_owned(), rect.right()));
            }
            Shape::Vec(shapes) => shapes.iter().for_each(|one| texts(one, found)),
            _ => {}
        }
    }

    fn painted(facts: &[Fact<'_>]) -> Vec<(String, f32)> {
        let mut harness = Harness::builder()
            .with_context_setup(crate::theme::apply)
            .with_size([WIDTH, 60.0])
            .build_ui(|ui| {
                FactsStrip::lay_out(ui, facts).show(ui);
            });
        harness.run_steps(2);
        let mut found = Vec::new();
        for clipped in &harness.output().shapes {
            texts(&clipped.shape, &mut found);
        }
        found
    }

    #[test]
    fn a_long_value_is_cut_and_the_facts_after_it_are_left_out_with_an_ellipsis() {
        let found = painted(&[
            Fact {
                key: "Alpha",
                value: "a value far too long to fit into the narrow bar",
            },
            Fact {
                key: "Beta",
                value: "1",
            },
            Fact {
                key: "Gamma",
                value: "2",
            },
        ]);

        assert!(
            found
                .iter()
                .any(|(text, right)| text.starts_with("a value") && *right > WIDTH / 2.0),
            "the value is shown, cut to the room there is: {found:?}"
        );
        assert!(
            found.iter().any(|(text, _)| text == "…"),
            "the bar ends in an ellipsis: {found:?}"
        );
        assert!(
            found
                .iter()
                .all(|(text, _)| !text.contains("BETA") && !text.contains("GAMMA")),
            "the later facts are left out: {found:?}"
        );
        assert!(
            found
                .iter()
                .all(|(_, right)| *right <= WIDTH - FACTS_PADDING_X),
            "nothing leaves the bar: {found:?}"
        );
    }

    #[test]
    fn facts_that_fit_are_all_shown_without_an_ellipsis() {
        let found = painted(&[
            Fact {
                key: "Alpha",
                value: "1",
            },
            Fact {
                key: "Beta",
                value: "2",
            },
        ]);

        assert!(
            found.iter().any(|(text, _)| text.contains("BETA")),
            "both: {found:?}"
        );
        assert!(
            found.iter().all(|(text, _)| !text.contains('…')),
            "no ellipsis: {found:?}"
        );
    }
}
