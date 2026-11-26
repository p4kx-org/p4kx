// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Response, Role, Sense, Ui, WidgetInfo, vec2};
use p4kx_extraction::{Entry, Texture};

use super::audio_preview;
use super::entry_status::status_body;
use super::preview_bar::Export;
use super::preview_tab::{Asked, Typed, typed_tab};
use super::record_preview;
use super::{dataforge_summary, image_preview, preview_bar, record_incoming};
use super::{soundbank_origins, soundbank_preview, source_origin};
use crate::theme::desktop::{PREVIEW_PADDING_X, PREVIEW_PATH_HEIGHT};
use crate::theme::palette;
use crate::theme::tokens::{FS_XS, SPACE_2, Weight};
use crate::widgets::frames;
use crate::widgets::scroll_region::Jump;
use crate::widgets::text::{hairline_below, line, paint_left, plain};
use crate::widgets::tooltip;
use crate::{
    AudioText, Browser, ComparisonText, Model, Preview, PreviewContents, PreviewTab,
    SoundbankPreview, Text, Texts, ViewerSupport, format, is_bank_path, is_model_path, is_wem_path,
};

/// Shows the Image tab after selecting a texture header, the Audio tab after selecting a WEM or a
/// soundbank, the Model tab after selecting a model, and leaves the XML, Image, Audio, Model,
/// DataForge, Record and Referenced-by tabs for the Text tab after selecting another Entry; the XML
/// tab opens once the Entry turns out to be `CryXml`.
pub(super) fn select(model: &mut Model, id: u32) {
    let path = model
        .browser()
        .and_then(|browser| browser.index().entry(id))
        .map(Entry::path);
    if path.is_some_and(Texture::is_header_path) {
        model.set_preview_tab(PreviewTab::Image);
    } else if path.is_some_and(|path| is_wem_path(path) || is_bank_path(path)) {
        model.set_preview_tab(PreviewTab::Audio);
    } else if path.is_some_and(is_model_path) {
        if model.preview_tab() != PreviewTab::ModelInfo {
            model.set_preview_tab(PreviewTab::Model);
        }
    } else if matches!(
        model.preview_tab(),
        PreviewTab::Xml
            | PreviewTab::Image
            | PreviewTab::Audio
            | PreviewTab::Model
            | PreviewTab::ModelInfo
            | PreviewTab::DataForge
            | PreviewTab::Record
            | PreviewTab::Incoming
    ) {
        model.set_preview_tab(PreviewTab::Text);
    }
}

pub(super) fn show(ui: &mut Ui, model: &mut Model) {
    load(ui, model);
    if model.preview_tab() == PreviewTab::Model
        && let Some(playback) = model.viewer_playback_mut()
    {
        let seconds = ui.input(|input| input.stable_dt);
        if playback.tick(seconds) {
            ui.ctx().request_repaint();
        }
    }
    let Some(browser) = model.browser() else {
        return;
    };
    let texts = model.texts();
    let export = if model.model_export_shown() {
        Some((Export::Model, model.model_export_available()))
    } else {
        model
            .interior_export_shown()
            .then(|| (Export::Interior, model.interior_export_available()))
    };
    let bar = preview_bar::tab_bar(ui, texts, browser, (model.preview_tab(), export));
    let tab = bar.tab;
    let asked = tab_body(
        ui,
        texts,
        browser,
        tab,
        (model.audio_volume(), model.viewer_support()),
    );
    if let Some(text) = bar.copy {
        ui.ctx().copy_text(text);
    }
    model.set_preview_tab(tab);
    match bar.export {
        Some(Export::Model) => model.begin_model_export(),
        Some(Export::Interior) => model.begin_interior_export(),
        None => {}
    }
    answer(ui, model, bar.step.map(Asked::Record).or(asked));
    if tab == PreviewTab::DataForge
        && let Some(browser) = model.browser_mut()
    {
        let ctx = ui.ctx().clone();
        browser.load_records(move || ctx.request_repaint());
    }
}

fn load(ui: &Ui, model: &mut Model) {
    if let Some(image) = model.browser_mut().and_then(Browser::image_mut) {
        image.upload(ui.ctx());
    }
    if model
        .browser_mut()
        .and_then(Browser::preview_mut)
        .is_some_and(Preview::take_xml_arrival)
    {
        model.set_preview_tab(PreviewTab::Xml);
    }
    let incoming = model.preview_tab() == PreviewTab::Incoming;
    if let Some(browser) = model.browser_mut() {
        let ctx = ui.ctx().clone();
        browser.load_record_preview({
            let ctx = ctx.clone();
            move || ctx.request_repaint()
        });
        if incoming {
            browser.load_incoming(move || ctx.request_repaint());
        }
    }
}

fn answer(ui: &Ui, model: &mut Model, asked: Option<Asked>) {
    match asked {
        Some(Asked::Mip(level)) => {
            if let Some(image) = model.browser_mut().and_then(Browser::image_mut) {
                image.request(level, image_preview::max_side(ui));
            }
        }
        Some(Asked::Audio(action)) => audio_preview::apply(model, action),
        Some(Asked::Viewer(ask)) => {
            if let Some(camera) = ask.camera
                && let Some(slot) = model.viewer_camera_mut()
            {
                *slot = camera;
            }
            if let Some(control) = ask.control
                && let Some(playback) = model.viewer_playback_mut()
            {
                playback.apply(control);
                ui.ctx().request_repaint();
            }
        }
        Some(Asked::Sound(id)) => {
            let ctx = ui.ctx().clone();
            if let Some(browser) = model.browser_mut() {
                browser.select_sound(id, move || ctx.request_repaint());
            }
        }
        Some(Asked::Record(step)) => record_preview::apply(model, &step),
        None => {}
    }
}

/// Shows the body of `tab`, the Audio tab at `volume` and the Model tab on `support`; returns what
/// the Image, Audio, Model or Record tab asks for.
fn tab_body(
    ui: &mut Ui,
    texts: &Texts,
    browser: &Browser,
    tab: PreviewTab,
    (volume, support): (f32, &ViewerSupport),
) -> Option<Asked> {
    let record = browser.selected_record().is_some();
    match (tab, browser.records()) {
        (PreviewTab::DataForge, Some(tree)) => {
            let size = browser.archive_index().dataforge_size();
            let _response = path_line(ui, tree.entry_path(), &format::size(size, texts.language()));
            dataforge_summary::show(ui, texts, tree);
            None
        }
        (PreviewTab::Record, Some(_)) if record => {
            record_preview::show(ui, texts, browser).map(Asked::Record)
        }
        (PreviewTab::Incoming, Some(_)) if record => {
            record_incoming::show(ui, texts, browser).map(Asked::Record)
        }
        (PreviewTab::Xml, Some(_)) if record => {
            record_preview::show_xml(ui, texts, browser);
            None
        }
        _ => body(ui, texts, browser, tab, (volume, support)),
    }
}

fn body(
    ui: &mut Ui,
    texts: &Texts,
    browser: &Browser,
    tab: PreviewTab,
    (volume, support): (f32, &ViewerSupport),
) -> Option<Asked> {
    let Some(preview) = browser.preview() else {
        frames::preview_body().show(ui, |ui| {
            ui.label(
                plain(texts.get(Text::PreviewNone), FS_XS, Weight::Regular)
                    .color(palette::GRAY_2_TEXT),
            );
        });
        return None;
    };
    let entry = browser.index().entry(preview.entry());
    let path = entry.map_or("", Entry::path);
    let size = entry.map_or(0, Entry::uncompressed_size);
    let size_line = preview
        .soundbank()
        .and_then(|bank| soundbank_preview::summary(texts, bank, size))
        .unwrap_or_else(|| format::size(size, texts.language()));
    let response = path_line(ui, path, &size_line);
    if matches!(tab, PreviewTab::Text | PreviewTab::Hex)
        && let Some(contents) = preview.contents()
    {
        entry_origins(response, texts, contents, (tab, preview.soundbank()));
    } else if let Some(bank) = preview.soundbank() {
        soundbank_origins::summary(response, texts, bank);
    }
    let jump = browser
        .hit_line()
        .filter(|hit| hit.entry() == preview.entry())
        .map(|hit| Jump {
            serial: hit.serial(),
            row: hit.row(),
            applied: browser.hit_applied(),
        });
    let typed = typed_tab(
        ui,
        texts,
        preview,
        (path, size),
        (tab, volume, support),
        jump,
    );
    if let Typed::Shown(asked) = typed {
        return asked;
    }
    status_body(ui, texts, preview, (path, size), (tab, jump));
    None
}

fn entry_origins(
    response: Response,
    texts: &Texts,
    contents: &PreviewContents,
    (tab, bank): (PreviewTab, Option<&SoundbankPreview>),
) {
    let proof = contents.provenance().ok();
    let (label, origin) = if tab == PreviewTab::Hex {
        (Text::PreviewTabHex, proof.map(|proof| &proof.hex))
    } else {
        (Text::PreviewTabText, proof.map(|proof| &proof.text))
    };
    tooltip::hover(response, |ui| {
        source_origin::row(
            ui,
            texts,
            ComparisonText::ColumnPath,
            proof.map(|proof| &proof.path),
        );
        source_origin::row(ui, texts, Text::ColumnSize, proof.map(|proof| &proof.size));
        source_origin::row(ui, texts, label, origin);
        if let Some(bank_proof) = bank.and_then(SoundbankPreview::summary_provenance) {
            source_origin::row(
                ui,
                texts,
                AudioText::BankWemCount,
                bank_proof.ok().map(|proof| &proof.wem_count),
            );
        }
    });
}

/// Shows the path line of the preview: `path`, then `size` muted.
pub(super) fn path_line(ui: &mut Ui, path: &str, size: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), PREVIEW_PATH_HEIGHT),
        Sense::hover(),
    );
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, path));
    let inner = rect.shrink2(vec2(PREVIEW_PADDING_X, 0.0));
    let size = line(ui, plain(size, FS_XS, Weight::Regular), f32::INFINITY);
    let galley = line(
        ui,
        plain(path, FS_XS, Weight::Regular),
        inner.width() - size.size().x - SPACE_2,
    );
    let size_rect = inner.with_min_x(inner.left() + galley.size().x + SPACE_2);
    paint_left(ui, inner, galley, palette::GRAY_1);
    paint_left(ui, size_rect, size, palette::GRAY_2_TEXT);
    hairline_below(ui, rect);
    response
}
