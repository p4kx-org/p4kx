// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use eframe::egui_wgpu::RendererOptions;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};

use super::test_fixture::Window;
use crate::dialog::Dialog;
use crate::ui::software_wgpu::software_wgpu_setup;
use crate::{Language, OutputTargetText as T};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn parent_and_name_dialogs_follow_the_desktop_spec_in_both_languages() -> TestResult {
    for (language, tag) in [
        (Language::BritishEnglish, "en-GB"),
        (Language::German, "de-DE"),
    ] {
        for size in [[640.0, 400.0], [1280.0, 800.0], [1920.0, 1080.0]] {
            for (dialog, invalid, kind) in [
                (Dialog::ModelFile, false, "file"),
                (Dialog::AssemblyPackage, false, "package"),
                (Dialog::DiagnosticCatalogFile, false, "diagnostic"),
                (Dialog::ModelFile, true, "invalid"),
            ] {
                draw(language, tag, size, dialog, invalid, kind)?;
            }
        }
    }
    Ok(())
}

fn draw(
    language: Language,
    tag: &str,
    size: [f32; 2],
    dialog: Dialog,
    invalid: bool,
    kind: &str,
) -> TestResult {
    let renderer = WgpuTestRenderer::from_render_state(create_render_state(
        software_wgpu_setup(),
        RendererOptions::PREDICTABLE,
    ));
    let mut harness = Harness::builder()
        .with_context_setup(crate::theme::apply)
        .with_size(size)
        .renderer(renderer)
        .build_ui_state(
            |ui, state: &mut Window| state.show(ui),
            Window::new(language, dialog, invalid),
        );
    harness.run();
    assert_eq!(
        harness.state().parent_requests,
        0,
        "rendering alone requests no folder"
    );
    assert_content(&harness, dialog, invalid);
    let image = harness.render()?;
    let name = format!(
        "output-target-{kind}-{tag}-{}x{}.png",
        image.width(),
        image.height()
    );
    save(&name, image.width(), image.height(), &image.into_raw())
}

fn assert_content(harness: &Harness<'_, Window>, dialog: Dialog, invalid: bool) {
    for text in [
        T::Title,
        T::Parent,
        T::Destination,
        if dialog.is_package_output() {
            T::PackageName
        } else {
            T::FileName
        },
    ] {
        let label = match text {
            T::FileName | T::PackageName => harness.state().texts.get(text).to_uppercase(),
            _ => harness.state().texts.get(text),
        };
        assert!(harness.query_by_label(&label).is_some(), "{label}");
    }
    if invalid {
        let label = harness.state().texts.get(T::Component);
        assert!(
            harness.query_by_label_contains(&label).is_some(),
            "invalid name alert"
        );
    }
    if dialog == Dialog::DiagnosticCatalogFile {
        let label = harness.state().texts.get(T::DiagnosticHint);
        assert!(
            harness.query_by_label_contains(&label).is_some(),
            "job-only diagnostic consent"
        );
    }
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
