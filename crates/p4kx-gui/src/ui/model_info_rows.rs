// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Ui;

use super::model_info::{Labelled, Table};
use crate::theme::desktop::{
    CELL_PADDING_X, DETAIL_COLUMN_GAP, DETAIL_KEY_MIN_WIDTH, DETAIL_KEY_TRACKING,
    DETAIL_VALUE_SIZE, PANEL_TITLE_SIZE, ROW_FONT, TABLE_HEAD_SIZE, TABLE_HEAD_TRACKING,
};
use crate::theme::tokens::{FS_SM, TRACKING_DISPLAY, Weight};
use crate::widgets::text::{caps, line, plain};
use crate::{
    AnimationCount, ChunkFile, ChunkNote, MaterialsInfo, ModelInfo, ModelInfoText as T, Texts,
    format,
};

const LISTED_IDS: usize = 24;

/// What the rows of the Info tab are laid out with.
pub(super) struct Rows<'a> {
    pub(super) ui: &'a Ui,
    pub(super) texts: &'a Texts,
    pub(super) width: f32,
}

impl Rows<'_> {
    fn number(&self, value: usize) -> String {
        format::number(
            u64::try_from(value).unwrap_or(u64::MAX),
            self.texts.language(),
        )
    }

    fn cell(&self, text: String, columns: usize) -> Labelled {
        let columns = u16::try_from(columns).map_or(1.0, f32::from).max(1.0);
        let width = 2.0f32
            .mul_add(-CELL_PADDING_X, self.width / columns)
            .max(0.0);
        let galley = line(self.ui, plain(&text, ROW_FONT, Weight::Regular), width);
        (text, galley)
    }

    fn head(&self, texts: &[T]) -> Vec<Labelled> {
        texts
            .iter()
            .map(|&text| {
                let name = self.texts.get(text);
                let rich = caps(&name, TABLE_HEAD_SIZE, Weight::Bold, TABLE_HEAD_TRACKING);
                let width = self.width / u16::try_from(texts.len()).map_or(1.0, f32::from);
                let inner = 2.0f32.mul_add(-CELL_PADDING_X, width).max(0.0);
                let galley = line(self.ui, rich, inner);
                (name, galley)
            })
            .collect()
    }

    fn heading(&self, text: String) -> Labelled {
        let rich = caps(&text, PANEL_TITLE_SIZE, Weight::Bold, TRACKING_DISPLAY);
        let galley = line(self.ui, rich, self.width);
        (text, galley)
    }
}

/// Returns the width of the key column and the key and value of every fact of `info`, the values
/// cut with an ellipsis at the column they are drawn in.
pub(super) fn facts(rows: &Rows<'_>, info: &ModelInfo) -> (f32, Vec<(Labelled, Labelled)>) {
    let texts = rows.texts;
    let geometry = info
        .geometry
        .clone()
        .unwrap_or_else(|| texts.get(T::GeometryInFile));
    let animations = match info.animations {
        AnimationCount::Reading => texts.get(T::AnimationsReading),
        AnimationCount::Counted(count) => rows.number(count),
        AnimationCount::TooMany | AnimationCount::Unread => texts.get(T::AnimationsUnread),
    };
    let chunks = texts.format(
        T::ChunksValue,
        &[
            ("total", rows.number(info.chunks.total).into()),
            ("read", rows.number(info.chunks.read).into()),
            ("skipped", rows.number(info.chunks.skipped).into()),
            ("unknown", rows.number(info.chunks.unknown).into()),
        ],
    );
    let facts = [
        (T::Nodes, rows.number(info.nodes)),
        (T::Meshes, rows.number(info.meshes())),
        (T::Lods, rows.number(info.lods.len())),
        (T::Geometry, geometry),
        (T::Materials, library(rows, info)),
        (T::MaterialIds, material_ids(rows, info)),
        (T::Bones, rows.number(info.bones)),
        (T::Animations, animations),
        (T::Chunks, chunks),
    ];
    let keys: Vec<Labelled> = facts
        .iter()
        .map(|&(name, _)| {
            let name = texts.get(name);
            let key_text = caps(&name, FS_SM, Weight::Bold, DETAIL_KEY_TRACKING);
            let galley = line(rows.ui, key_text, f32::INFINITY);
            (name, galley)
        })
        .collect();
    let key_width = keys
        .iter()
        .map(|(_, galley)| galley.size().x)
        .fold(DETAIL_KEY_MIN_WIDTH, f32::max);
    let value_width = (rows.width - key_width - DETAIL_COLUMN_GAP).max(0.0);
    let facts = keys
        .into_iter()
        .zip(facts)
        .map(|(key, (_, value))| {
            let value_text = plain(&value, DETAIL_VALUE_SIZE, Weight::Light);
            let galley = line(rows.ui, value_text, value_width);
            (key, (value, galley))
        })
        .collect();
    (key_width, facts)
}

fn library(rows: &Rows<'_>, info: &ModelInfo) -> String {
    let texts = rows.texts;
    let name = info.library.clone().unwrap_or_default();
    let library = ("library", name.into());
    match &info.materials {
        MaterialsInfo::Unnamed => texts.get(T::MaterialsUnnamed),
        MaterialsInfo::Missing => texts.format(T::MaterialsMissing, &[library]),
        MaterialsInfo::Unreadable(reason) => texts.format(
            T::MaterialsUnreadable,
            &[library, ("reason", reason.clone().into())],
        ),
        MaterialsInfo::Loaded { single: true, .. } => texts.format(T::MaterialsSingle, &[library]),
        MaterialsInfo::Loaded { materials, .. } => texts.format(
            T::MaterialsLoaded,
            &[
                library,
                (
                    "count",
                    u64::try_from(*materials).unwrap_or(u64::MAX).into(),
                ),
                ("number", rows.number(*materials).into()),
            ],
        ),
    }
}

fn material_ids(rows: &Rows<'_>, info: &ModelInfo) -> String {
    if info.material_ids.is_empty() {
        return rows.texts.get(T::MaterialIdsNone);
    }
    let mut ids: Vec<String> = info
        .material_ids
        .iter()
        .take(LISTED_IDS)
        .map(u16::to_string)
        .collect();
    if info.material_ids.len() > LISTED_IDS {
        ids.push("…".to_owned());
    }
    ids.join(", ")
}

/// Returns the table of the levels of detail and, when the model left chunks unread, the table of
/// those.
pub(super) fn tables(rows: &Rows<'_>, info: &ModelInfo) -> Vec<Table> {
    let texts = rows.texts;
    let columns = [
        T::ColumnLod,
        T::ColumnVertices,
        T::ColumnTriangles,
        T::ColumnMeshes,
    ];
    let lods = info
        .lods
        .iter()
        .map(|lod| {
            [
                lod.level.to_string(),
                rows.number(lod.vertices),
                rows.number(lod.triangles),
                rows.number(lod.meshes),
            ]
            .into_iter()
            .map(|text| rows.cell(text, columns.len()))
            .collect()
        })
        .collect();
    let mut tables = vec![Table {
        heading: rows.heading(texts.get(T::LodsHeading)),
        head: rows.head(&columns),
        rows: lods,
    }];
    if !info.unread.is_empty() {
        tables.push(unread(rows, info));
    }
    tables
}

fn unread(rows: &Rows<'_>, info: &ModelInfo) -> Table {
    let texts = rows.texts;
    let columns = [
        T::ColumnFile,
        T::ColumnType,
        T::ColumnTypeId,
        T::ColumnUsage,
    ];
    let number = info.chunks.skipped + info.chunks.unknown;
    let cells = info
        .unread
        .iter()
        .map(|chunk| {
            [
                file_name(texts, chunk),
                chunk
                    .name
                    .map_or_else(|| texts.get(T::TypeUnknown), str::to_owned),
                chunk.type_id.clone(),
                texts.get(if chunk.unknown {
                    T::UsageUnknown
                } else {
                    T::UsageSkipped
                }),
            ]
            .into_iter()
            .map(|text| rows.cell(text, columns.len()))
            .collect()
        })
        .collect();
    Table {
        heading: rows
            .heading(texts.format(T::ChunksHeading, &[("number", rows.number(number).into())])),
        head: rows.head(&columns),
        rows: cells,
    }
}

/// Returns the name of the file holding `chunk` in the current language.
pub(crate) fn file_name(texts: &Texts, chunk: &ChunkNote) -> String {
    match chunk.file {
        ChunkFile::Model => texts.get(T::FileModel),
        ChunkFile::Geometry => texts.get(T::FileGeometry),
        ChunkFile::LodGeometry(level) => {
            texts.format(T::FileLod, &[("level", level.to_string().into())])
        }
        ChunkFile::LodModel(level) => {
            texts.format(T::FileLodModel, &[("level", level.to_string().into())])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::file_name;
    use crate::{ChunkFile, ChunkNote, Language, Texts};

    #[test]
    fn level_model_and_geometry_files_have_distinct_localised_names() {
        for (language, model, geometry) in [
            (
                Language::BritishEnglish,
                "LOD 3 model file",
                "LOD 3 geometry",
            ),
            (
                Language::German,
                "Modelldatei von LOD 3",
                "Geometrie von LOD 3",
            ),
        ] {
            let texts = Texts::new(language);
            let mut note = ChunkNote {
                file: ChunkFile::LodModel(3),
                index: 0,
                name: None,
                type_id: "0xDEADBEEF".to_owned(),
                unknown: true,
            };
            assert_eq!(
                file_name(&texts, &note),
                model,
                "level model label resolves its Fluent argument"
            );
            note.file = ChunkFile::LodGeometry(3);
            assert_eq!(
                file_name(&texts, &note),
                geometry,
                "existing level geometry label is unchanged"
            );
        }
    }
}
