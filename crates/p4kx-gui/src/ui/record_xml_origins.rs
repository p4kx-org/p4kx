// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Label, Response, RichText, ScrollArea, Sense, Ui};

use crate::shown_line::display;
use crate::theme::desktop::XML_ROW_HEIGHT;
use crate::ui::source_origin;
use crate::widgets::tooltip;
use crate::{RecordXmlFieldProvenance, Texts, XmlDocument};

const HOVER_FIELDS: usize = 6;
const MENU_FIELD_ROWS: f32 = 18.0;

#[derive(Clone, Copy)]
pub(super) enum Evidence<'a> {
    Unrequested,
    Unknown,
    Fields(&'a [RecordXmlFieldProvenance]),
}

impl Evidence<'_> {
    pub(super) fn sense(self) -> Sense {
        if matches!(self, Self::Fields(_)) {
            Sense::click()
        } else {
            Sense::hover()
        }
    }

    pub(super) fn show(
        self,
        response: Response,
        texts: &Texts,
        document: &XmlDocument,
        line: usize,
    ) {
        match self {
            Self::Unrequested => (),
            Self::Unknown => {
                tooltip::hover(response, |ui| {
                    tooltip::line(ui, texts.get(source_origin::label(None)));
                });
            }
            Self::Fields(fields) => {
                let Some(fields) = line_fields(fields, document, line) else {
                    return;
                };
                response.context_menu(|ui| evidence_menu(ui, texts, document, fields));
                if response.hovered() {
                    tooltip::hover(response, |ui| {
                        for field in fields.iter().take(HOVER_FIELDS) {
                            evidence_row(ui, texts, document, field);
                        }
                        if fields.len() > HOVER_FIELDS {
                            ui.label("…");
                        }
                    });
                }
            }
        }
    }
}

fn line_fields<'a>(
    fields: &'a [RecordXmlFieldProvenance],
    document: &XmlDocument,
    line: usize,
) -> Option<&'a [RecordXmlFieldProvenance]> {
    let line = document.line_range(line)?;
    let start = line.start as u64;
    let end = line.end as u64;
    let first =
        fields.partition_point(|field| field.bytes.start < start && field.bytes.end <= start);
    let last = fields.partition_point(|field| field.bytes.start < end);
    fields.get(first..last).filter(|slice| !slice.is_empty())
}

fn evidence_menu(
    ui: &mut Ui,
    texts: &Texts,
    document: &XmlDocument,
    fields: &[RecordXmlFieldProvenance],
) {
    ScrollArea::vertical()
        .max_height(XML_ROW_HEIGHT * MENU_FIELD_ROWS)
        .show_rows(ui, XML_ROW_HEIGHT, fields.len(), |ui, visible| {
            for field in fields.get(visible).unwrap_or_default() {
                evidence_row(ui, texts, document, field);
            }
        });
}

fn evidence_row(
    ui: &mut Ui,
    texts: &Texts,
    document: &XmlDocument,
    field: &RecordXmlFieldProvenance,
) {
    let fragment = usize::try_from(field.bytes.start)
        .ok()
        .zip(usize::try_from(field.bytes.end).ok())
        .and_then(|(start, end)| document.bytes().get(start..end));
    let text = match fragment {
        Some([]) => String::from("\"\""),
        Some(bytes) => display(bytes).text,
        None => String::new(),
    };
    ui.horizontal(|ui| {
        tooltip::line(ui, texts.get(source_origin::label(Some(&field.origin))));
        ui.add(Label::new(RichText::new(text).monospace()).truncate());
    });
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::SourceUses;

    use super::{RecordXmlFieldProvenance, XmlDocument, line_fields};

    #[test]
    fn a_line_exposes_every_span_including_empty_values_at_its_start() {
        let origin = SourceUses::new().origin(&[], false);
        let document = XmlDocument::new(b"first\nsecond\n".to_vec(), false);
        let fields: Vec<_> = [0..5, 6..6, 6..8, 8..12]
            .into_iter()
            .map(|bytes| RecordXmlFieldProvenance {
                bytes,
                origin: origin.clone(),
            })
            .collect();
        assert_eq!(
            line_fields(&fields, &document, 0).map(<[_]>::len),
            Some(1),
            "first line"
        );
        assert_eq!(
            line_fields(&fields, &document, 1).map(<[_]>::len),
            Some(3),
            "empty value belongs with the next line"
        );
        assert!(
            line_fields(&fields, &document, 2).is_none(),
            "no line beyond retained XML"
        );
    }
}
