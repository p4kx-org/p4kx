// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use egui::{Galley, Id, Rect, ScrollArea, Sense, Ui, vec2};

use p4kx_extraction::{FieldOrigin, MaterialSummaryDetailOrigin, MaterialSummaryOrigins};

use super::model_info_rows::{Rows, facts, tables};
use super::{model_preview, source_origin};
use crate::theme::desktop::{
    DETAIL_COLUMN_GAP, DETAIL_ROW_GAP, PANEL_TITLE_MARGIN_BOTTOM, PANEL_TITLE_MARGIN_TOP,
    PANEL_TITLE_PADDING_BOTTOM,
};
use crate::theme::palette;
use crate::widgets::frames;
use crate::widgets::text::hairline_below;
use crate::widgets::tooltip;
use crate::{
    AnimationCountProvenance, Language, ModelInfo, ModelInfoProvenance, ModelInfoText as T,
    ModelPreview, Texts,
};

#[path = "model_info_grid.rs"]
mod grid;

#[derive(Clone, Copy)]
struct Origins<'a> {
    model: Option<&'a ModelInfoProvenance>,
    animation_count: Option<&'a FieldOrigin>,
}

pub(super) type Labelled = (String, Arc<Galley>);
pub(super) struct Fact {
    pub(super) name: T,
    pub(super) key: Labelled,
    pub(super) value: Labelled,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Key {
    serial: u64,
    language: Language,
    width: f32,
    pixels_per_point: f32,
}

/// The laid-out info of one model, kept across frames until the model, the language or the width
/// change.
pub(super) struct Laid {
    key: Key,
    key_width: f32,
    pub(super) facts: Vec<Fact>,
    pub(super) tables: Vec<Table>,
}

/// A heading and a table below it.
pub(super) struct Table {
    pub(super) heading: Labelled,
    pub(super) head: Vec<Labelled>,
    pub(super) rows: Vec<Vec<Labelled>>,
    pub(super) kind: TableKind,
}

#[derive(Clone, Copy)]
pub(super) enum TableKind {
    Lods,
    UnreadChunks,
}

/// Shows the Info tab of the model at `path`: what its file holds, or why there is nothing to
/// list.
pub(super) fn show(ui: &mut Ui, texts: &Texts, path: &str, preview: &ModelPreview) {
    let Some(info) = preview.info() else {
        model_preview::unread(ui, texts, path, preview.status());
        return;
    };
    let origins = Origins {
        model: preview.info_provenance().and_then(Result::ok),
        animation_count: preview
            .animation_count_provenance()
            .and_then(Result::ok)
            .map(AnimationCountProvenance::origin),
    };
    frames::preview_body().show(ui, |ui| {
        ScrollArea::vertical()
            .id_salt("model-info")
            .auto_shrink(false)
            .show(ui, |ui| Laid::get(ui, texts, info).show(ui, texts, origins));
    });
}

impl Laid {
    fn get(ui: &Ui, texts: &Texts, info: &ModelInfo) -> Arc<Self> {
        let id = Id::unique("p4kx-model-info");
        let key = Key {
            serial: info.serial(),
            language: texts.language(),
            width: ui.available_width(),
            pixels_per_point: ui.ctx().pixels_per_point(),
        };
        if let Some(laid) = ui.data(|data| data.get_temp::<Arc<Self>>(id))
            && laid.key == key
        {
            return laid;
        }
        let rows = Rows {
            ui,
            texts,
            width: key.width,
        };
        let (key_width, facts) = facts(&rows, info);
        let laid = Arc::new(Self {
            key,
            key_width,
            facts,
            tables: tables(&rows, info),
        });
        ui.data_mut(|data| data.insert_temp(id, Arc::clone(&laid)));
        laid
    }

    fn show(&self, ui: &mut Ui, texts: &Texts, origins: Origins<'_>) {
        ui.spacing_mut().item_spacing.y = 0.0;
        let key_width = self.key_width;
        for (index, data) in self.facts.iter().enumerate() {
            if index > 0 {
                ui.add_space(DETAIL_ROW_GAP);
            }
            fact(ui, index, data, key_width, texts, origins);
        }
        for (index, table) in self.tables.iter().enumerate() {
            ui.add_space(PANEL_TITLE_MARGIN_TOP);
            let size = vec2(
                ui.available_width(),
                table.heading.1.size().y + PANEL_TITLE_PADDING_BOTTOM,
            );
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let text_rect = rect.with_max_y(rect.bottom() - PANEL_TITLE_PADDING_BOTTOM);
            grid::labelled(
                ui,
                (1, index, 0),
                &table.heading,
                (text_rect, palette::GRAY_1),
            );
            hairline_below(ui, rect);
            ui.add_space(PANEL_TITLE_MARGIN_BOTTOM);
            grid::show(ui, index, table, texts, origins.model);
        }
    }
}

fn fact(
    ui: &mut Ui,
    index: usize,
    Fact { name, key, value }: &Fact,
    key_width: f32,
    texts: &Texts,
    Origins {
        model: origins,
        animation_count,
    }: Origins<'_>,
) {
    let proof = fact_origin(*name, origins);

    let height = key.1.size().y.max(value.1.size().y) + DETAIL_ROW_GAP;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let content = rect.with_max_y(rect.bottom() - DETAIL_ROW_GAP);
    let key_rect = rect.with_max_x(rect.left() + key_width);
    let value_rect = rect.with_min_x(key_rect.right() + DETAIL_COLUMN_GAP);
    let on_baseline = |cell: Rect, galley: &Galley| {
        cell.with_min_y(content.bottom() - galley.size().y)
            .with_max_y(content.bottom())
    };
    grid::labelled(
        ui,
        (0, index, 0),
        key,
        (on_baseline(key_rect, &key.1), palette::GRAY_2_TEXT),
    );
    let shown = grid::labelled(
        ui,
        (0, index, 1),
        value,
        (on_baseline(value_rect, &value.1), palette::DATA_FG),
    );
    let proof =
        proof.or_else(|| (*name == T::Animations).then_some((T::Animations, animation_count)));
    if value.1.elided || proof.is_some() || matches!(name, T::Materials | T::Chunks) {
        tooltip::hover(shown, |ui| {
            if value.1.elided {
                tooltip::line(ui, value.0.as_str());
            }
            if let Some((field, origin)) = proof {
                source_origin::row(ui, texts, field, origin);
            }
            match name {
                T::Materials => material_origins(
                    ui,
                    texts,
                    origins.map(ModelInfoProvenance::material_library),
                ),
                T::Chunks => chunk_count_origins(ui, texts, origins),
                _ => {}
            }
        });
    }
    hairline_below(ui, key_rect);
    hairline_below(ui, value_rect);
}

fn fact_origin(
    name: T,
    origins: Option<&ModelInfoProvenance>,
) -> Option<(T, Option<&FieldOrigin>)> {
    let summary = origins.map(ModelInfoProvenance::mesh_summary);
    let topology = summary.map(|summary| &summary.topology);
    let (label, origin) = match name {
        T::Nodes => (name, origins.map(ModelInfoProvenance::node_count)),
        T::Meshes => (name, topology.and_then(|proof| proof.meshes_by_lod.first())),
        T::Lods => (name, topology.map(|proof| &proof.lod_count)),
        T::Geometry => (name, topology.map(|proof| &proof.geometry)),
        T::Bones => (name, origins.map(ModelInfoProvenance::bone_count)),
        T::MaterialIds => (
            T::ListedMaterialIds,
            summary.map(|proof| &proof.material_ids),
        ),

        _ => return None,
    };
    Some((label, origin))
}

fn chunk_count_origins(ui: &mut Ui, texts: &Texts, origins: Option<&ModelInfoProvenance>) {
    let usage = origins.map(ModelInfoProvenance::chunk_usage);
    for (label, origin) in [
        (
            T::ChunksTotal,
            origins.map(ModelInfoProvenance::chunk_total),
        ),
        (T::ChunksRead, usage.map(|origin| &origin.read)),
        (T::ChunksSkipped, usage.map(|origin| &origin.skipped)),
        (T::ChunksUnknown, usage.map(|origin| &origin.unknown)),
    ] {
        source_origin::row(ui, texts, label, origin);
    }
}

fn material_origins(ui: &mut Ui, texts: &Texts, origins: Option<&MaterialSummaryOrigins>) {
    if let Some(path) = origins.and_then(|summary| summary.path.as_ref()) {
        source_origin::row(ui, texts, T::Materials, Some(path));
    }
    source_origin::row(
        ui,
        texts,
        T::MaterialsStatus,
        origins.map(|summary| &summary.status),
    );
    let (label, origin) = match origins.and_then(|summary| summary.detail.as_ref()) {
        Some(MaterialSummaryDetailOrigin::Single(origin)) => (T::MaterialSingle, origin),
        Some(MaterialSummaryDetailOrigin::Count(origin)) => (T::MaterialCount, origin),
        _ => return,
    };
    source_origin::row(ui, texts, label, Some(origin));
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use egui::{CentralPanel, Context, RawInput};
    use p4kx_extraction::ModelSource;
    use p4kx_testkit::TestModel;

    use super::Laid;
    use crate::theme::desktop::DETAIL_COLUMN_GAP;
    use crate::{Language, ModelInfo, Texts};

    fn info() -> Result<ModelInfo, Box<dyn std::error::Error>> {
        let folder = tempfile::tempdir()?;
        let cube = TestModel::cube();
        std::fs::write(folder.path().join("cube.cgf"), cube.cgf())?;
        std::fs::write(folder.path().join("cube.cgfm"), cube.cgfm())?;
        let source = ModelSource::from_file(&folder.path().join("cube.cgf"))?;
        Ok(ModelInfo::of(source.model()))
    }

    fn laid(ctx: &Context, texts: &Texts, info: &ModelInfo) -> Arc<Laid> {
        let mut shown = None;
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            CentralPanel::default().show(ui, |ui| {
                shown = Some(Laid::get(ui, texts, info));
            });
        });
        output.textures_delta.clear();
        shown.unwrap_or_else(|| unreachable!("the closure ran"))
    }

    #[test]
    fn the_laid_out_info_is_formatted_once_per_model_and_language()
    -> Result<(), Box<dyn std::error::Error>> {
        let ctx = Context::default();
        crate::theme::apply(&ctx);
        let mut texts = Texts::new(Language::BritishEnglish);
        let info = info()?;

        let first = laid(&ctx, &texts, &info);
        let second = laid(&ctx, &texts, &info);
        texts.set_language(Language::German);
        let german = laid(&ctx, &texts, &info);
        let other = laid(&ctx, &texts, &ModelInfo::clone(&info));

        assert!(Arc::ptr_eq(&first, &second), "the same frame after frame");
        assert!(
            !Arc::ptr_eq(&second, &german),
            "another language is laid out again"
        );
        assert!(
            Arc::ptr_eq(&german, &other),
            "a copy of the same model is the same"
        );
        Ok(())
    }

    #[test]
    fn a_long_value_is_cut_with_an_ellipsis_inside_its_column()
    -> Result<(), Box<dyn std::error::Error>> {
        let ctx = Context::default();
        crate::theme::apply(&ctx);
        let texts = Texts::new(Language::BritishEnglish);
        let mut info = info()?;
        info.library = Some("objects/".repeat(200));

        let laid = laid(&ctx, &texts, &info);

        let widest = laid
            .facts
            .iter()
            .map(|fact| fact.value.1.size().x)
            .fold(0.0, f32::max);
        assert!(laid.facts.iter().any(|fact| fact.value.1.elided), "elided");
        assert!(
            widest + laid.key_width + DETAIL_COLUMN_GAP <= ctx.content_rect().width(),
            "{widest} fits next to the key column"
        );
        Ok(())
    }
}
