// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Response, Ui};
use fluent_bundle::FluentValue;

use crate::ui::extraction_outcome::list_row_response;
use crate::ui::record_export_outcome;
use crate::ui::source_origin::row;
use crate::widgets::tooltip;
use crate::{
    BreakLine, ImageText, SoundList, SoundListProvenance, SoundsTab, SoundsText as T, Text, Texts,
    format,
};

pub(super) fn proof(tab: &SoundsTab) -> Option<&SoundListProvenance> {
    tab.resolution_provenance().and_then(Result::ok)
}

pub(super) fn heading(response: Response, texts: &Texts, proof: Option<&SoundListProvenance>) {
    tooltip::hover(response, |ui| {
        row(
            ui,
            texts,
            T::ColumnTrigger,
            proof.map(|proof| &proof.trigger),
        );
        row(
            ui,
            texts,
            T::ScrollVariants,
            proof.map(|proof| &proof.variant_count),
        );
    });
}

pub(super) fn variant(
    response: Response,
    texts: &Texts,
    proof: Option<&SoundListProvenance>,
    position: usize,
) {
    let found = proof.and_then(|proof| proof.variants().get(position));
    tooltip::hover(response, |ui| {
        for (label, field) in [
            (
                Text::from(T::ColumnEvent),
                found.map(|source| &source.event),
            ),
            (T::ColumnWem.into(), found.map(|source| &source.wem_id)),
            (T::ColumnSource.into(), found.map(|source| &source.kind)),
            (
                T::ColumnLanguage.into(),
                found.map(|source| &source.language),
            ),
            (
                T::ColumnSoundbank.into(),
                found.map(|source| &source.soundbank_path),
            ),
            (ImageText::Size.into(), found.map(|source| &source.size)),
            (T::ColumnState.into(), found.map(|source| &source.playable)),
        ] {
            row(ui, texts, label, field);
        }
    });
}

pub(super) fn broken_links(
    ui: &mut Ui,
    texts: &Texts,
    list: &SoundList,
    proof: Option<&SoundListProvenance>,
) {
    let count = u64::try_from(list.breaks.len()).unwrap_or(u64::MAX);
    let heading = texts.format(
        T::BreaksHeading,
        &[
            ("count", FluentValue::from(count)),
            ("number", format::number(count, texts.language()).into()),
        ],
    );
    record_export_outcome::list_rows(
        ui,
        (&heading, "sound-breaks"),
        list.breaks.len(),
        false,
        |ui, at| {
            let Some(link) = list.breaks.get(at) else {
                return;
            };
            let text = break_line(texts, link);
            let response = list_row_response(ui, &text, None);
            let found = proof.and_then(|proof| proof.breaks().get(at));
            tooltip::hover(response, |ui| {
                row(ui, texts, T::ColumnEvent, found.map(|source| &source.event));
                row(
                    ui,
                    texts,
                    T::ColumnSoundbank,
                    found.map(|source| &source.soundbank),
                );
                row(
                    ui,
                    texts,
                    T::ColumnState,
                    found.map(|source| &source.reason),
                );
            });
        },
    );
}

fn break_line(texts: &Texts, link: &BreakLine) -> String {
    let mut args = vec![
        ("name", link.event.clone().into()),
        ("reason", link.reason.clone().into()),
    ];
    let text = link.soundbank.as_ref().map_or(T::BreakLine, |bank| {
        args.push(("bank", bank.clone().into()));
        T::BreakLineIn
    });
    texts.format(text, &args)
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use p4kx_extraction::SourceUses;

    use super::{ImageText, T, Texts, row, tooltip};
    use crate::Language;

    #[test]
    fn sound_origin_tooltip_shows_each_field_state_with_the_desktop_theme() {
        for language in [Language::German, Language::BritishEnglish] {
            let texts = Texts::new(language);
            let uses = SourceUses::new();
            let clear = uses.origin(&[], true);
            let mut harness = Harness::builder()
                .with_context_setup(crate::theme::apply)
                .build_ui(|ui| {
                    tooltip::frame().show(ui, |ui| {
                        row(ui, &texts, T::ColumnEvent, Some(&clear));
                        row(ui, &texts, T::ColumnSource, None);
                    });
                });
            harness.run();
            for label in [
                texts.get(T::ColumnEvent),
                texts.get(T::ColumnSource),
                texts.get(ImageText::OriginPlain),
                texts.get(ImageText::OriginUnknown),
            ] {
                let _found = harness.get_by_label(&label);
            }
        }
    }
}
