// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Color32, Label, ScrollArea, Ui, vec2};
use fluent_bundle::FluentValue;

use super::regions;
use super::search::eyebrow;
use super::ui_tab_variables;
use super::{Dialog, Requests};
use crate::theme::desktop::{SEGMENT_PADDING_X, SPINNER_INLINE, TOOLBAR_PADDING};
use crate::theme::palette;
use crate::theme::tokens::{FS_SM, FS_XS, Weight};
use crate::theme::ui_metrics::UI_TOOLBAR_MAX_SHARE;
use crate::widgets::button::{self, Kind, Size};
use crate::widgets::indicator;
use crate::widgets::tab;
use crate::widgets::text::{line, plain};
use crate::{
    Model, Texts, UiBackground, UiExport, UiExportStatus, UiExportSummary, UiFormat, UiMode,
    UiPower, UiSettings, UiSize, UiText, format,
};

/// A labelled group of segments.
struct Cluster {
    label: UiText,
    items: Vec<UiText>,
    active: usize,
}

impl Cluster {
    fn of<T: Copy + PartialEq>(
        label: UiText,
        (all, current): (&[T], T),
        text: impl Fn(T) -> UiText,
    ) -> Self {
        Self {
            label,
            items: all.iter().map(|item| text(*item)).collect(),
            active: all.iter().position(|item| *item == current).unwrap_or(0),
        }
    }

    fn width(&self, ui: &Ui, texts: &Texts) -> f32 {
        let label = line(ui, eyebrow(&texts.get(self.label)), f32::INFINITY)
            .size()
            .x;
        let segments: f32 = self
            .items
            .iter()
            .map(|item| {
                let name = plain(texts.get(*item), FS_XS, Weight::Bold);
                2.0f32.mul_add(SEGMENT_PADDING_X, line(ui, name, f32::INFINITY).size().x)
            })
            .sum();
        label + TOOLBAR_PADDING + segments
    }

    fn show(&self, ui: &mut Ui, texts: &Texts) -> Option<usize> {
        let names: Vec<String> = self.items.iter().map(|item| texts.get(*item)).collect();
        ui.label(eyebrow(&texts.get(self.label)));
        let clicked = tab::segments(ui, &names, Some(self.active));
        ui.add_space(TOOLBAR_PADDING);
        clicked
    }
}

/// What the toolbar was used for.
#[derive(Default)]
struct Chosen {
    mode: Option<UiMode>,
    size: Option<UiSize>,
    background: Option<UiBackground>,
    power: Option<UiPower>,
    format: Option<UiFormat>,
    export: bool,
}

/// What a position of the toolbar holds.
#[derive(Clone, Copy)]
enum Slot {
    Mode,
    Size,
    Background,
    Power,
    Format,
}

/// Shows the choice of the source, of the size, background and format, and the export, then the
/// state of the export.
pub(super) fn show(ui: &mut Ui, model: &mut Model, requests: &mut Requests) {
    ui.spacing_mut().item_spacing = vec2(TOOLBAR_PADDING, TOOLBAR_PADDING);
    let width = ui.available_width();
    let chosen = ScrollArea::both()
        .id_salt("ui-toolbar")
        .max_height(ui.ctx().content_rect().height() * UI_TOOLBAR_MAX_SHARE)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            regions::clip(ui);
            choices(ui, model, width)
        })
        .inner;
    if let Some(mode) = chosen.mode {
        model.ui_mut().set_mode(mode);
    }
    if let Some(size) = chosen.size {
        model.ui_mut().set_size(size);
        requests.settings_changed = true;
    }
    if let Some(background) = chosen.background {
        model.ui_mut().set_background(background);
        requests.settings_changed = true;
    }
    if let Some(power) = chosen.power {
        model.ui_mut().set_power(power);
        requests.settings_changed = true;
    }
    if let Some(format) = chosen.format {
        model.ui_mut().set_format(format);
        requests.settings_changed = true;
    }
    if chosen.export {
        requests.dialog = Some(Dialog::UiFile);
    }
    if model.ui().mode() == UiMode::Ships {
        requests.settings_changed |= ui_tab_variables::show(ui, model);
    }
    outcome(ui, model);
}

/// Returns the groups of segments the tab offers, in the order of the toolbar; the game state
/// only for ships.
fn slots(model: &Model) -> Vec<(Slot, Cluster)> {
    let tab = model.ui();
    let UiSettings {
        size,
        background,
        format,
        power,
    } = *tab.settings();
    let mut slots = vec![
        (
            Slot::Mode,
            Cluster::of(
                UiText::ModeLabel,
                (&[UiMode::Files, UiMode::Ships], tab.mode()),
                |mode| match mode {
                    UiMode::Files => UiText::ModeFiles,
                    UiMode::Ships => UiText::ModeShips,
                },
            ),
        ),
        (
            Slot::Size,
            Cluster::of(UiText::SizeLabel, (&UiSize::ALL, size), UiSize::text),
        ),
        (
            Slot::Background,
            Cluster::of(
                UiText::BackgroundLabel,
                (&UiBackground::ALL, background),
                UiBackground::text,
            ),
        ),
    ];
    if tab.mode() == UiMode::Ships {
        slots.push((
            Slot::Power,
            Cluster::of(UiText::PowerLabel, (&UiPower::ALL, power), UiPower::text),
        ));
    }
    slots.push((
        Slot::Format,
        Cluster::of(
            UiText::FormatLabel,
            (&UiFormat::ALL, format),
            UiFormat::text,
        ),
    ));
    slots
}

impl Chosen {
    /// Notes that segment `picked` of the group `slot` was clicked.
    fn take(&mut self, slot: Slot, picked: Option<usize>) {
        match slot {
            Slot::Mode => self.mode = picked.and_then(mode_at),
            Slot::Size => self.size = picked.and_then(|at| UiSize::ALL.get(at).copied()),
            Slot::Background => {
                self.background = picked.and_then(|at| UiBackground::ALL.get(at).copied());
            }
            Slot::Power => self.power = picked.and_then(|at| UiPower::ALL.get(at).copied()),
            Slot::Format => self.format = picked.and_then(|at| UiFormat::ALL.get(at).copied()),
        }
    }
}

fn choices(ui: &mut Ui, model: &Model, width: f32) -> Chosen {
    let texts = model.texts();
    let slots = slots(model);
    let export = texts.get(UiText::Export);
    let button_width = button::width(ui, Size::Normal, &export);
    let mut widths: Vec<f32> = slots
        .iter()
        .map(|(_, cluster)| cluster.width(ui, texts))
        .collect();
    widths.push(button_width);
    let rows = pack(&widths, width, TOOLBAR_PADDING);
    let mut chosen = Chosen::default();
    let available = model.ui().export_available();
    for row in rows {
        ui.horizontal(|ui| {
            for position in row {
                let Some((slot, cluster)) = slots.get(position) else {
                    chosen.export =
                        button::button(ui, Kind::Cta, Size::Normal, &export, available).clicked();
                    continue;
                };
                chosen.take(*slot, cluster.show(ui, texts));
            }
        });
    }
    chosen
}

fn mode_at(at: usize) -> Option<UiMode> {
    [UiMode::Files, UiMode::Ships].get(at).copied()
}

/// Splits items of the given widths into rows that each fit `available`, in order.
pub(super) fn pack(widths: &[f32], available: f32, gap: f32) -> Vec<std::ops::Range<usize>> {
    let mut rows = Vec::new();
    let (mut start, mut used) = (0, 0.0_f32);
    for (position, width) in widths.iter().enumerate() {
        let needed = if position == start {
            *width
        } else {
            used + gap + width
        };
        if position > start && needed > available {
            rows.push(start..position);
            (start, used) = (position, *width);
        } else {
            used = needed;
        }
    }
    rows.push(start..widths.len());
    rows
}

fn with_file<'a>(
    file: &str,
    extra: &[(&'a str, FluentValue<'a>)],
) -> Vec<(&'a str, FluentValue<'a>)> {
    let mut all = vec![("file", FluentValue::from(file.to_owned()))];
    all.extend(extra.iter().cloned());
    all
}

fn outcome(ui: &mut Ui, model: &mut Model) {
    let texts = model.texts();
    let Some(export) = model.ui().export() else {
        return;
    };
    let (text, tone, running) = status_line(texts, export);
    let dismiss = texts.get(UiText::ExportDismiss);
    let mut dismissed = false;
    ui.horizontal_wrapped(|ui| {
        if running {
            indicator::spinner(ui, SPINNER_INLINE);
        }
        ui.add(Label::new(plain(text, FS_SM, Weight::Regular).color(tone)).selectable(true));
        if !running {
            dismissed = button::button(ui, Kind::Ghost, Size::Compact, &dismiss, true).clicked();
        }
    });
    if dismissed {
        model.ui_mut().dismiss_export();
    }
}

fn done_line(texts: &Texts, file: &str, summary: &UiExportSummary) -> (String, Color32) {
    let size = [
        ("width", FluentValue::from(u64::from(summary.width))),
        ("height", FluentValue::from(u64::from(summary.height))),
    ];
    let mut all = with_file(file, &size);
    if summary.left_out == 0 {
        return (
            texts.format(UiText::ExportDone, &all),
            palette::SUCCESS_TEXT,
        );
    }
    let count = u64::try_from(summary.left_out).unwrap_or(u64::MAX);
    all.push(("count", FluentValue::from(count)));
    all.push((
        "number",
        FluentValue::from(format::number(count, texts.language())),
    ));
    (
        texts.format(UiText::ExportDoneIncomplete, &all),
        palette::WARNING,
    )
}

fn status_line(texts: &Texts, export: &UiExport) -> (String, Color32, bool) {
    let file = export.file().display().to_string();
    match export.status() {
        UiExportStatus::Running => (
            texts.format(UiText::ExportRunning, &with_file(&file, &[])),
            palette::GRAY_1,
            true,
        ),
        UiExportStatus::Done(summary) => {
            let (text, tone) = done_line(texts, &file, summary);
            (text, tone, false)
        }
        UiExportStatus::DoneArchiveChanged(_) => (
            texts.format(UiText::ExportWrittenArchiveChanged, &with_file(&file, &[])),
            palette::WARNING,
            false,
        ),
        UiExportStatus::Refused(refused) => (
            crate::installation::message(texts, refused),
            palette::DANGER_TEXT,
            false,
        ),
        UiExportStatus::IsArchive => (
            texts.get(UiText::ExportIsArchive),
            palette::DANGER_TEXT,
            false,
        ),
        UiExportStatus::ArchiveChanged => (
            texts.get(UiText::ExportArchiveChanged),
            palette::WARNING,
            false,
        ),
        UiExportStatus::Failed(Some(reason)) => (
            texts.format(
                UiText::ExportFailed,
                &[("reason", reason.to_owned().into())],
            ),
            palette::DANGER_TEXT,
            false,
        ),
        UiExportStatus::Failed(None) => (
            texts.get(UiText::ExportStopped),
            palette::DANGER_TEXT,
            false,
        ),
    }
}
