// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Align, Color32, Label, Layout, Ui};
use fluent_bundle::FluentValue;

use p4kx_extraction::Build;

use crate::theme::desktop::{SPINNER_INLINE, STATUS_ITEM_PADDING_X, STATUS_PATH_MAX_WIDTH};
use crate::theme::palette;
use crate::theme::tokens::{FS_XS, Weight};
use crate::widgets::indicator;
use crate::widgets::text::{link, plain};
use crate::{Browser, ContainerText, Model, Place, Text, Texts, format};

/// Shows the status bar, the desktop density of `.footer`.
pub(super) fn show(ui: &mut Ui, model: &Model) {
    let texts = model.texts();
    ui.spacing_mut().item_spacing.x = STATUS_ITEM_PADDING_X;
    ui.horizontal_centered(|ui| {
        if let Some(path) = model.opening_path() {
            indicator::spinner(ui, SPINNER_INLINE);
            let path = path.display().to_string();
            item(
                ui,
                texts.format(Text::StatusOpening, &[("path", path.into())]),
                palette::PRIMARY,
            );
        } else if let Some(failure) = model.failure() {
            let path = failure.path.display().to_string();
            let text = match &failure.reason {
                Some(reason) => texts.format(
                    Text::StatusFailed,
                    &[("path", path.into()), ("reason", reason.clone().into())],
                ),
                None => texts.format(Text::StatusOpeningStopped, &[("path", path.into())]),
            };
            item(ui, text, palette::DANGER_TEXT);
        } else if model.browser().is_none() {
            item(ui, texts.get(Text::StatusEmpty), palette::GRAY_2_TEXT);
        }
        if let Some(browser) = model.browser() {
            browser_items(ui, texts, browser);
        }
        if let Some(release) = model.newer_release() {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let text = texts.format(
                    Text::UpdateAvailable,
                    &[("version", release.version.clone().into())],
                );
                link(ui, &text, release.link());
            });
        }
    });
}

fn item(ui: &mut Ui, text: String, colour: Color32) {
    ui.add(
        Label::new(plain(text, FS_XS, Weight::Regular).color(colour))
            .truncate()
            .selectable(false),
    );
}

fn browser_items(ui: &mut Ui, texts: &Texts, browser: &Browser) {
    let language = texts.language();
    let opened = browser.archive_index();
    let entries_text = if browser.place() == Place::Archive {
        Text::StatusEntries
    } else {
        Text::from(ContainerText::StatusEntries)
    };
    let count = |value: usize| u64::try_from(value).unwrap_or(u64::MAX);
    let number =
        |value: usize| -> FluentValue<'static> { format::number(count(value), language).into() };
    ui.scope(|ui| {
        ui.set_max_width(STATUS_PATH_MAX_WIDTH);
        item(ui, opened.path().display().to_string(), palette::GRAY_1);
    });
    if opened.is_dataforge_file() {
        return;
    }
    let parts = [
        build_label(texts, opened.build_of_archive()),
        texts.format(
            entries_text,
            &[
                ("count", count(browser.index().len()).into()),
                ("number", number(browser.index().len())),
            ],
        ),
        texts.format(Text::StatusShown, &[("number", number(browser.view_len()))]),
        selected_label(texts, browser, number),
    ];
    for part in parts {
        ui.separator();
        item(ui, part, palette::GRAY_2_TEXT);
    }
    if browser.is_searching() {
        ui.separator();
        indicator::spinner(ui, SPINNER_INLINE);
        item(ui, texts.get(Text::StatusSearching), palette::PRIMARY);
    }
}

fn selected_label(
    texts: &Texts,
    browser: &Browser,
    number: impl Fn(usize) -> FluentValue<'static>,
) -> String {
    let selected = ("number", number(browser.checked_count()));
    match browser.hidden_checked_count() {
        0 => texts.format(Text::StatusSelected, &[selected]),
        hidden => texts.format(
            Text::StatusSelectedHidden,
            &[selected, ("hidden", number(hidden))],
        ),
    }
}

/// Returns `build` as "Build <version>", or that the Build is unknown.
pub(super) fn build_label(texts: &Texts, build: Option<&Build>) -> String {
    build.map_or_else(
        || texts.get(Text::StatusBuildUnknown),
        |build| {
            texts.format(
                Text::StatusBuild,
                &[("build", build.version().to_owned().into())],
            )
        },
    )
}
