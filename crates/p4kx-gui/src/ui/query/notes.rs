// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Color32, Galley, Id, Response, Role, Sense, TextStyle, TextWrapMode, Ui, WidgetInfo, WidgetText,
};
use fluent_bundle::FluentValue;

use crate::theme::palette;
use crate::theme::tokens::{FS_SM, FS_XS, Weight, font};
use crate::ui::search::eyebrow;
use crate::ui::shortcuts::{RUN_QUERY, command_hint};
use crate::widgets::pagination::PageNames;
use crate::widgets::text::{line, plain};
use crate::{
    QueryExportStatus, QueryResult, QueryStage, QueryStatus, QueryTab, QueryText, Texts, format,
};

/// The laid-out texts of the Query tab, made again only when the status, the result, the
/// export, the language or the width change.
pub(super) struct Notes {
    key: u64,
    label: (Arc<str>, Arc<Galley>),
    pub(super) hint: Arc<str>,
    pub(super) run: Arc<str>,
    pub(super) cancel: Arc<str>,
    pub(super) save: Arc<str>,
    pub(super) export_csv: Arc<str>,
    pub(super) export_json: Arc<str>,
    pub(super) export_cancel: Arc<str>,
    pub(super) status: Option<Arc<Galley>>,
    pub(super) fraction: Option<f32>,
    pub(super) alert: Option<Arc<Galley>>,
    pub(super) prompt: Option<Arc<Galley>>,
    pub(super) count: Option<Arc<Galley>>,
    pub(super) export: Option<Arc<Galley>>,
    pub(super) page_status: Option<Arc<Galley>>,
    page_names: [Arc<str>; 5],
}

impl Notes {
    /// Returns the texts for `query`, laid out again when anything they show changed.
    pub(super) fn get(ui: &Ui, texts: &Texts, query: &QueryTab) -> Arc<Self> {
        let mut hasher = DefaultHasher::new();
        texts.language().hash(&mut hasher);
        ui.ctx().os().is_mac().hash(&mut hasher);
        status_key(query, &mut hasher);
        ui.available_width().to_bits().hash(&mut hasher);
        ui.ctx().pixels_per_point().to_bits().hash(&mut hasher);
        let key = hasher.finish();
        let id = Id::unique("p4kx-query-notes");
        if let Some(notes) = ui.data(|data| data.get_temp::<Arc<Self>>(id))
            && notes.key == key
        {
            return notes;
        }
        let notes = Arc::new(make(ui, texts, query, key));
        ui.data_mut(|data| data.insert_temp(id, Arc::clone(&notes)));
        notes
    }

    /// Shows the label of the editor.
    pub(super) fn show_label(&self, ui: &mut Ui) -> Response {
        labelled(ui, &self.label)
    }

    /// Returns the accessible names of the pagination.
    pub(super) fn page_names(&self) -> PageNames<'_> {
        let [first, previous, next, last, status] = &self.page_names;
        PageNames {
            first,
            previous,
            next,
            last,
            status,
        }
    }
}

fn query_prompt(ui: &Ui, texts: &Texts) -> String {
    let shortcut = command_hint(ui, RUN_QUERY, || texts.get(QueryText::RunShortcut));
    texts.format(QueryText::Prompt, &[("shortcut", shortcut.as_str().into())])
}

fn make(ui: &Ui, texts: &Texts, query: &QueryTab, key: u64) -> Notes {
    let shared = |text: QueryText| Arc::<str>::from(texts.get(text));
    let label = texts.get(QueryText::Label);
    let label_galley = line(ui, eyebrow(&label), f32::INFINITY);
    let width = ui.available_width();
    let (status, fraction) = status(texts, query.status())
        .map_or((None, None), |(text, colour, fraction)| {
            (Some(note(ui, text, colour)), fraction)
        });
    let alert = alert(ui, texts, query.status(), width);
    let result = query.result();
    let prompt = match (query.status(), result) {
        (QueryStatus::Idle, _) => Some(query_prompt(ui, texts)),
        (_, Some(result)) if result.is_empty() => Some(texts.get(QueryText::NoRows)),
        _ => None,
    }
    .map(|text| wrapped(ui, text, FS_SM, palette::GRAY_2_TEXT, width));
    let page = result.map(|result| (result.page(), result.page_count()));
    Notes {
        key,
        label: (label.into(), label_galley),
        hint: shared(QueryText::Hint),
        run: shared(QueryText::Run),
        cancel: shared(QueryText::Cancel),
        save: shared(QueryText::Save),
        export_csv: shared(QueryText::ExportCsv),
        export_json: shared(QueryText::ExportJson),
        export_cancel: shared(QueryText::ExportCancel),
        status,
        fraction,
        alert,
        prompt,
        count: result.map(|result| {
            let text = plain(count(texts, result), FS_SM, Weight::Regular);
            line(ui, text.color(palette::GRAY_2_TEXT), f32::INFINITY)
        }),
        export: query
            .export_status()
            .map(|export| export_note(texts, export))
            .map(|(text, colour)| note(ui, text, colour)),
        page_status: page.map(|(page, pages)| page_status(ui, page, pages)),
        page_names: page_names(texts, page),
    }
}

fn alert(ui: &Ui, texts: &Texts, status: QueryStatus<'_>, width: f32) -> Option<Arc<Galley>> {
    let QueryStatus::Invalid(invalid) = status else {
        return None;
    };
    let text = texts.format(
        QueryText::Invalid,
        &[
            ("line", invalid.line().into()),
            ("column", invalid.column().into()),
            ("reason", invalid.message().to_owned().into()),
        ],
    );
    Some(wrapped(ui, text, FS_SM, palette::GRAY_1, width))
}

fn page_names(texts: &Texts, page: Option<(usize, usize)>) -> [Arc<str>; 5] {
    let shared = |text: QueryText| Arc::<str>::from(texts.get(text));
    let status = page.map_or_else(String::new, |(page, pages)| {
        texts.format(
            QueryText::Page,
            &[("page", (page + 1).into()), ("pages", pages.into())],
        )
    });
    [
        shared(QueryText::PageFirst),
        shared(QueryText::PagePrevious),
        shared(QueryText::PageNext),
        shared(QueryText::PageLast),
        status.into(),
    ]
}

fn number(texts: &Texts, value: usize) -> FluentValue<'static> {
    format::number(u64::try_from(value).unwrap_or(u64::MAX), texts.language()).into()
}

fn status(texts: &Texts, status: QueryStatus<'_>) -> Option<(String, Color32, Option<f32>)> {
    Some(match status {
        QueryStatus::Running(QueryStage::Loading) => {
            (texts.get(QueryText::Loading), palette::GRAY_1, None)
        }
        QueryStatus::Running(QueryStage::Indexing) => {
            (texts.get(QueryText::Indexing), palette::GRAY_1, None)
        }
        QueryStatus::Running(QueryStage::Selecting) => {
            (texts.get(QueryText::Selecting), palette::GRAY_1, None)
        }
        QueryStatus::Running(QueryStage::Reading { done, total }) => {
            let percent = done
                .saturating_mul(100)
                .checked_div(total)
                .unwrap_or(100)
                .min(100);
            let text = texts.format(
                QueryText::Progress,
                &[
                    ("done", number(texts, done)),
                    ("total", number(texts, total)),
                    ("percent", percent.to_string().into()),
                ],
            );
            let fraction = f32::from(u8::try_from(percent).unwrap_or(100)) / 100.0;
            (text, palette::GRAY_1, Some(fraction))
        }
        QueryStatus::Cancelled => (texts.get(QueryText::Cancelled), palette::GRAY_2_TEXT, None),
        QueryStatus::Failed(Some(reason)) => (
            texts.format(QueryText::Failed, &[("reason", reason.to_owned().into())]),
            palette::DANGER_TEXT,
            None,
        ),
        QueryStatus::Failed(None) => (texts.get(QueryText::Stopped), palette::DANGER_TEXT, None),
        QueryStatus::ArchiveChanged => {
            (texts.get(QueryText::ArchiveChanged), palette::WARNING, None)
        }
        QueryStatus::Idle | QueryStatus::Invalid(_) | QueryStatus::Finished(_) => return None,
    })
}

fn count(texts: &Texts, result: &QueryResult) -> String {
    let rows = result.len();
    let count = (
        "count",
        FluentValue::from(u64::try_from(rows).unwrap_or(u64::MAX)),
    );
    let shown = ("number", number(texts, rows));
    let mut text = if result.failed() == 0 {
        texts.format(QueryText::Rows, &[count, shown])
    } else {
        let failed = format::number(result.failed(), texts.language());
        texts.format(
            QueryText::RowsFailed,
            &[count, shown, ("failed", failed.into())],
        )
    };
    let unresolved = result.unresolved_references();
    if unresolved > 0 {
        let shown = format::number(unresolved, texts.language());
        text.push_str(&texts.format(
            QueryText::Unresolved,
            &[
                ("count", FluentValue::from(unresolved)),
                ("number", shown.into()),
            ],
        ));
    }
    text
}

fn export_note(texts: &Texts, export: &QueryExportStatus) -> (String, Color32) {
    let path = |path: &std::path::Path| ("path", path.display().to_string().into());
    match export {
        QueryExportStatus::Exporting(at) => (
            texts.format(QueryText::Exporting, &[path(at)]),
            palette::PRIMARY,
        ),
        QueryExportStatus::Exported {
            path: at, records, ..
        } => (
            texts.format(
                QueryText::Exported,
                &[
                    ("count", FluentValue::from(*records)),
                    ("number", format::number(*records, texts.language()).into()),
                    path(at),
                ],
            ),
            palette::GRAY_2_TEXT,
        ),
        QueryExportStatus::Failed(at, reason) => (
            texts.format(
                QueryText::ExportFailed,
                &[path(at), ("reason", reason.clone().into())],
            ),
            palette::DANGER_TEXT,
        ),
        QueryExportStatus::Cancelled(at) => (
            texts.format(QueryText::ExportCancelled, &[path(at)]),
            palette::GRAY_2_TEXT,
        ),
    }
}

/// Returns `status` laid out as the `.page-status`: the current page in bold orange, then the
/// number of pages.
fn page_status(ui: &Ui, page: usize, pages: usize) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.append(
        &(page + 1).to_string(),
        0.0,
        TextFormat::simple(font(FS_XS, Weight::Bold), palette::PRIMARY),
    );
    job.append(
        &format!(" / {pages}"),
        0.0,
        TextFormat::simple(font(FS_XS, Weight::Regular), palette::GRAY_1),
    );
    ui.ctx().fonts_mut(|fonts| fonts.layout_job(job))
}

/// Lays `text` out in `colour`, wrapped at `width`.
fn wrapped(ui: &Ui, text: String, size: f32, colour: Color32, width: f32) -> Arc<Galley> {
    WidgetText::from(plain(text, size, Weight::Regular).color(colour)).into_galley(
        ui,
        Some(TextWrapMode::Wrap),
        width,
        TextStyle::Body,
    )
}

/// Lays `text` out on one line in `colour` at the note size.
fn note(ui: &Ui, text: String, colour: Color32) -> Arc<Galley> {
    line(
        ui,
        plain(text, FS_XS, Weight::Regular).color(colour),
        f32::INFINITY,
    )
}

/// Shows the label of the editor as an eyebrow named by its Fluent text.
fn labelled(ui: &mut Ui, (name, galley): &(Arc<str>, Arc<Galley>)) -> Response {
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &**name));
    ui.painter()
        .galley(rect.min, Arc::clone(galley), palette::GRAY_2_TEXT);
    response
}

/// Hashes what decides the texts of the tab.
fn status_key(query: &QueryTab, hasher: &mut impl Hasher) {
    match query.status() {
        QueryStatus::Idle => 0_u8.hash(hasher),
        QueryStatus::Running(stage) => {
            1_u8.hash(hasher);
            stage.hash(hasher);
        }
        QueryStatus::Invalid(invalid) => {
            2_u8.hash(hasher);
            invalid.hash(hasher);
        }
        QueryStatus::Cancelled => 3_u8.hash(hasher),
        QueryStatus::Failed(reason) => {
            4_u8.hash(hasher);
            reason.hash(hasher);
        }
        QueryStatus::ArchiveChanged => 5_u8.hash(hasher),
        QueryStatus::Finished(result) => {
            6_u8.hash(hasher);
            result.generation().hash(hasher);
        }
    }
    query.export_status().hash(hasher);
}
