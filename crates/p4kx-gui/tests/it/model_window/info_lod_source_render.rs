// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::viewing::{self, Site, TestResult};
use p4kx_gui::{Language, Unavailable, ViewerSupport};
use p4kx_testkit::{TestEntry, TestModel};

use super::info_render;

const UNKNOWN_MODEL_CHUNK: u32 = 0xDEAD_BEEF;
const UNKNOWN_GEOMETRY_CHUNK: u32 = 0xCAFE_BABE;
const SCROLL_EVENTS: usize = 6;

fn site() -> Result<Site, Box<dyn std::error::Error>> {
    let level = TestModel::two_bone_skin()
        .with_chunk(UNKNOWN_MODEL_CHUNK, [7, 8, 9])
        .with_geometry_chunk(UNKNOWN_GEOMETRY_CHUNK, [10, 11, 12]);
    let model = TestModel::two_bone_skin().with_lod(level);
    Ok(Site {
        entries: vec![
            TestEntry::zstd("Data/Objects/skin.skin", model.cgf()),
            TestEntry::zstd("Data/Objects/skin.skinm", model.cgfm()),
            TestEntry::zstd(
                "Data/Objects/skin_lod1.skin",
                model.lod_cgf(1).ok_or("level model fixture")?,
            ),
            TestEntry::zstd(
                "Data/Objects/skin_lod1.skinm",
                model.lod_cgfm(1).ok_or("level geometry fixture")?,
            ),
        ],
        installation: false,
        key: false,
    })
}

#[test]
fn lod_model_and_geometry_chunk_sources_render_distinctly_in_both_languages() -> TestResult {
    use crate::waiting::RunQuiet;
    use egui_kittest::kittest::Queryable;

    for (language, suffix, model_label, geometry_label) in [
        (
            Language::BritishEnglish,
            "en-GB",
            "LOD 1 model file",
            "LOD 1 geometry",
        ),
        (
            Language::German,
            "de-DE",
            "Modelldatei von LOD 1",
            "Geometrie von LOD 1",
        ),
    ] {
        let renderer = info_render::renderer().ok_or("software renderer unavailable")?;
        let support = ViewerSupport::Unavailable(Unavailable::NoDevice);
        let (_folder, mut harness) = viewing::opened_at(language, support, site()?, |builder| {
            builder.renderer(renderer)
        })?;
        viewing::select(&mut harness, "skin.skin")?;
        harness.get_by_label("Info").click();
        harness.run_quiet();
        harness
            .get_by_label_contains(info_render::rows_label(language))
            .hover();
        for _ in 0..SCROLL_EVENTS {
            harness.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, -4.0),
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Wheel,
                modifiers: egui::Modifiers::NONE,
            });
            harness.run_quiet();
        }
        assert!(
            harness.query_all_by_label(model_label).next().is_some(),
            "model source"
        );
        assert!(
            harness.query_all_by_label(geometry_label).next().is_some(),
            "geometry source"
        );
        info_render::save(
            &format!("model-info-lod-sources-{suffix}.png"),
            &mut harness,
        )?;
    }
    Ok(())
}
