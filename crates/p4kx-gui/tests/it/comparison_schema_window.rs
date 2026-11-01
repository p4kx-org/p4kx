// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The schema list of the Comparison tab drawn and driven headless over a pair of synthetic
//! DataForges, and rendered by wgpu when a graphics adapter exists.
//!
//! `P4KX_RENDER_DIR` names a folder the rendered window is written to as a PNG file.

use crate::record_pair;
use crate::waiting::{RunQuiet, wait_for_model_described};

use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::{Path, PathBuf};

use crate::software_wgpu::software_wgpu_setup;
use eframe::egui_wgpu::RendererOptions;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_gui::{
    ComparisonList, ComparisonSide, ComparisonStatus, ComparisonView, Language, Model, WorkTab, ui,
};
use record_pair::{BOTH, TestResult, pair};

fn harness(
    (old, new): (&Path, &Path),
    language: Language,
    renderer: Option<WgpuTestRenderer>,
) -> Harness<'static, Model> {
    let mut model = Model::new(language);
    model.set_work_tab(WorkTab::Comparison);
    let tab = model.comparison_mut();
    tab.set_archive(ComparisonSide::Old, old.to_path_buf());
    tab.set_archive(ComparisonSide::New, new.to_path_buf());
    tab.set_levels(BOTH);
    let builder = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1280.0, 900.0]);
    let builder = match renderer {
        Some(renderer) => builder.renderer(renderer),
        None => builder,
    };
    builder.build_ui_state(
        |ui, model: &mut Model| {
            ui::show(ui, model);
        },
        model,
    )
}

fn schema_shown(
    pair: (&Path, &Path),
    language: Language,
    renderer: Option<WgpuTestRenderer>,
) -> Result<Harness<'static, Model>, Box<dyn Error>> {
    let mut harness = harness(pair, language, renderer);
    harness.run_quiet();
    let compare = if language == Language::German {
        "Vergleichen"
    } else {
        "Compare"
    };
    harness.get_by_label(compare).click();
    wait_for_model_described(
        &mut harness,
        "the Comparison to finish",
        |model| matches!(model.comparison().status(), ComparisonStatus::Finished(_)),
        |model| format!("{:?}", model.comparison().status()),
    )?;
    harness.run_quiet();
    harness.get_by_role_and_label(Role::Tab, "Schema").click();
    harness.run_quiet();
    Ok(harness)
}

fn save(name: &str, width: u32, height: u32, image: &[u8]) -> TestResult {
    let Some(folder) = std::env::var_os("P4KX_RENDER_DIR").map(PathBuf::from) else {
        return Ok(());
    };
    std::fs::create_dir_all(&folder)?;
    let mut encoder = png::Encoder::new(
        BufWriter::new(File::create(folder.join(name))?),
        width,
        height,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(image)?;
    Ok(())
}

#[test]
fn the_schema_tab_lists_the_structs_properties_and_enums_that_differ() -> TestResult {
    let (_dir, old, new) = pair()?;

    let harness = schema_shown((&old, &new), Language::BritishEnglish, None)?;

    assert_eq!(
        harness
            .state()
            .comparison()
            .view()
            .map(ComparisonView::list),
        Some(ComparisonList::Schema),
        "the tab switched the list"
    );
    for row in [
        "Removed: Struct Legacy",
        "Changed: Struct Probe",
        "Changed: Property level",
        "Added: Property tag",
        "Added: Enum Rating",
    ] {
        assert!(harness.query_by_label(row).is_some(), "the row {row}");
    }
    assert!(
        harness
            .query_by_label("Structs: 0 added, 1 removed, 1 changed")
            .is_some(),
        "the structs are counted"
    );
    assert!(
        harness
            .query_by_label("Properties: 1 added, 0 removed, 1 changed")
            .is_some(),
        "the properties are counted"
    );
    Ok(())
}

#[test]
fn the_schema_tab_is_translated() -> TestResult {
    let (_dir, old, new) = pair()?;

    let harness = schema_shown((&old, &new), Language::German, None)?;

    assert!(
        harness
            .query_by_label("Structs: 0 hinzugefügt, 1 entfernt, 1 geändert")
            .is_some(),
        "the counts in German"
    );
    assert!(
        harness
            .query_by_label_contains("Eigenschaft level")
            .is_some(),
        "the kind in German"
    );
    Ok(())
}

#[expect(
    clippy::print_stderr,
    reason = "tells the test run why a test checked nothing"
)]
fn renderer() -> Option<WgpuTestRenderer> {
    let state =
        catch_unwind(|| create_render_state(software_wgpu_setup(), RendererOptions::PREDICTABLE))
            .ok();
    if state.is_none() {
        eprintln!("no graphics adapter: the test checks nothing");
    }
    state.map(WgpuTestRenderer::from_render_state)
}

#[test]
fn the_schema_list_is_drawn() -> TestResult {
    let Some(renderer) = renderer() else {
        return Ok(());
    };
    let (_dir, old, new) = pair()?;
    let mut harness = schema_shown((&old, &new), Language::BritishEnglish, Some(renderer))?;
    harness.run_steps(3);

    let image = harness.render()?;
    save(
        "comparison-schema.png",
        image.width(),
        image.height(),
        image.as_raw(),
    )?;
    Ok(())
}
