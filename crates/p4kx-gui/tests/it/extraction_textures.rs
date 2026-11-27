// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The texture outputs of the Extraction setup, driven headless through `egui_kittest`.

use crate::common::settle;
use crate::extraction_outputs::{assert_outputs, assert_receipted_outputs};
use crate::waiting::wait_for_model;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_gui::{Language, Model, ui};
use p4kx_settings::ExtractionTextures;
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestTextureFormat};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

const PANEL: &str = "Data\\Textures\\panel_diff.dds";
const BROKEN: &str = "Data\\Textures\\broken_ddna.dds";
const MERGE: &str = "Merge split textures into one DDS";
const PNG: &str = "Export textures as PNG (high-range as OpenEXR)";
const RAW: &str = "Keep the raw texture parts";

fn panel() -> SplitDds {
    SplitDds::new(TestTextureFormat::LegacyBgra8, 16, 8).split(2)
}

fn opened(
    textures: ExtractionTextures,
    broken: bool,
) -> Result<(TempDir, PathBuf, Harness<'static, Model>), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let mut entries = panel().entries(PANEL);
    if broken {
        let mut parts = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8)
            .split(1)
            .entries(BROKEN);
        parts.truncate(1);
        entries.extend(parts);
    }
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    let out = dir.path().join("out");
    fs::create_dir(&out)?;
    let mut model = Model::new(Language::BritishEnglish);
    model.set_extraction_folder(Some(out.clone()));
    model.set_extraction_conversion(true);
    model.set_extraction_textures(textures);
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1400.0, 1200.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        );
    harness.state_mut().open(path, || {});
    wait_until(&mut harness, |model| model.browser().is_some())?;
    Ok((dir, out, harness))
}

fn wait_until(harness: &mut Harness<'_, Model>, done: impl Fn(&Model) -> bool) -> TestResult {
    wait_for_model(harness, "the window to settle", done)?;
    settle(harness);
    Ok(())
}

fn extract_shown(harness: &mut Harness<'_, Model>, clicks: &[&str]) -> TestResult {
    harness.get_by_label("Extract shown…").click();
    settle(harness);
    harness.get_by_label("Existing folder").click();
    settle(harness);
    for label in clicks {
        harness.get_by_label(label).click();
        settle(harness);
    }
    harness.get_by_label("Extract").click();
    settle(harness);
    harness.get_by_label("Extract whole Archive").click();
    wait_until(harness, |model| {
        model
            .extraction()
            .is_some_and(|extraction| !extraction.is_running())
    })
}

fn files(out: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let folder = out.join("Data").join("Textures");
    let mut names: Vec<String> = fs::read_dir(folder)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()?;
    names.sort();
    Ok(names)
}

#[test]
fn the_default_outputs_merge_the_texture_and_export_its_largest_mip() -> TestResult {
    let (_dir, out, mut harness) = opened(ExtractionTextures::default(), false)?;

    extract_shown(&mut harness, &[])?;

    harness.get_by_label("Extracted 3 (3 converted), skipped 0, failed 0, cancelled 0");
    harness.get_by_label("DDS origin receipts (1)");
    assert_outputs(
        &out.join("Data/Textures"),
        &["panel_diff.dds"],
        &["panel_diff.png"],
    )?;
    let merged = fs::read(out.join("Data/Textures/panel_diff.dds"))?;
    assert_eq!(
        merged.len(),
        128 + (16 * 8 + 8 * 4 + 4 * 2 + 2 + 1) * 4,
        "the whole chain"
    );
    Ok(())
}

#[test]
fn the_switches_choose_the_outputs_and_are_remembered() -> TestResult {
    let (_dir, out, mut harness) = opened(ExtractionTextures::default(), false)?;

    extract_shown(&mut harness, &[PNG, RAW])?;

    assert_receipted_outputs(
        &out.join("Data/Textures"),
        &["panel_diff.dds", "panel_diff.dds.1", "panel_diff.dds.2"],
    )?;
    assert_eq!(
        harness.state().extraction_textures(),
        ExtractionTextures {
            merge: true,
            png: false,
            mip: 0,
            keep_raw: true,
        },
        "remembered for the next setup"
    );
    Ok(())
}

#[test]
fn the_setup_starts_with_the_stored_outputs_and_mip_level() -> TestResult {
    let stored = ExtractionTextures {
        merge: false,
        png: true,
        mip: 1,
        keep_raw: false,
    };
    let (_dir, out, mut harness) = opened(stored, false)?;

    harness.get_by_label("Extract shown…").click();
    settle(&mut harness);
    assert_eq!(
        harness
            .state()
            .extraction_setup()
            .map(p4kx_gui::ExtractionSetup::textures),
        Some(stored),
        "preset from the settings"
    );
    harness.get_by_label("Close").click();
    settle(&mut harness);
    extract_shown(&mut harness, &[])?;

    assert_eq!(files(&out)?, ["panel_diff.png"], "only the PNG");
    let png = fs::read(out.join("Data/Textures/panel_diff.png"))?;
    let width = png.get(16..20).map(<[u8]>::to_vec);
    assert_eq!(width, Some(vec![0, 0, 0, 8]), "mip 1 is 8 pixels wide");
    Ok(())
}

#[test]
fn a_texture_that_cannot_be_converted_is_listed_with_its_reason() -> TestResult {
    let (_dir, out, mut harness) = opened(ExtractionTextures::default(), true)?;

    extract_shown(&mut harness, &[MERGE])?;

    harness.get_by_label("Not converted (1)");
    harness.get_by_label(&format!(
        "{BROKEN}: cannot convert the texture: {BROKEN}.1 is missing"
    ));
    assert!(
        files(&out)?.contains(&"broken_ddna.dds".to_owned()),
        "the raw header"
    );
    Ok(())
}

#[test]
fn choosing_a_mip_level_exports_a_smaller_mip() -> TestResult {
    let (_dir, out, mut harness) = opened(ExtractionTextures::default(), false)?;

    extract_shown(&mut harness, &["Mip level"])?;

    assert_eq!(harness.state().extraction_textures().mip, 1, "level 1");
    let png = fs::read(out.join("Data/Textures/panel_diff.png"))?;
    let width = png.get(16..20).map(<[u8]>::to_vec);
    assert_eq!(width, Some(vec![0, 0, 0, 8]), "mip 1 is 8 pixels wide");
    Ok(())
}
