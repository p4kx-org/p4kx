// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::PathBuf;

use eframe::egui_wgpu::RendererOptions;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::wgpu::{self, WgpuTestRenderer};
use p4kx_extraction::{AnimationSet, Archive, GlbOptions, ModelSource};
use p4kx_testkit::{self as testkit, ArchiveBuilder, TestEntry};

use crate::theme::desktop::{BUTTON_HEIGHT, ROW_HEIGHT};
use crate::ui::software_wgpu;
use crate::widgets::placement;
use crate::{ExtractionText, Language, ModelExportSummary, Texts};

type TestResult = Result<(), Box<dyn Error>>;

const REFERENCE: &str = concat!(
    "//host/branch/Data/Objects/RenderFixture/",
    "InteriorPanels/SyntheticMaterialCollection/",
    "BrushedMetalPaintedTrimWarningLabels.mtl"
);
const TARGET: &str = concat!(
    "Data/Objects/RenderFixture/InteriorPanels/SyntheticMaterialCollection/",
    "BrushedMetalPaintedTrimWarningLabels.mtl"
);
const VIEWPORT: [f32; 2] = [1280.0, 800.0];

#[derive(Clone, Copy)]
enum Case {
    MappedMissing,
    MappedUnreadable,
}

impl Case {
    const fn tag(self) -> &'static str {
        match self {
            Self::MappedMissing => "mapped-missing",
            Self::MappedUnreadable => "mapped-unreadable",
        }
    }
}

struct Window {
    texts: Texts,
    summary: ModelExportSummary,
}

#[test]
fn mapped_missing_material_notes_wrap_in_the_real_summary_window() -> TestResult {
    render(Case::MappedMissing)
}

#[test]
fn mapped_unreadable_material_notes_wrap_in_the_real_summary_window() -> TestResult {
    render(Case::MappedUnreadable)
}

fn render(case: Case) -> TestResult {
    for (language, tag) in [
        (Language::BritishEnglish, "en-GB"),
        (Language::German, "de-DE"),
    ] {
        let window = fixture(language, case)?;
        let laid = super::Laid::new((1, language), &window.texts, &window.summary);
        let note = laid
            .notes
            .iter()
            .find(|(text, _)| text.contains(TARGET))
            .map(|(text, _)| text.clone())
            .ok_or("mapped material note missing from the production summary")?;
        let renderer = WgpuTestRenderer::from_render_state(wgpu::create_render_state(
            software_wgpu::software_wgpu_setup(),
            RendererOptions::PREDICTABLE,
        ));
        let mut harness = Harness::builder()
            .with_context_setup(crate::theme::apply)
            .with_size(VIEWPORT)
            .renderer(renderer)
            .build_ui_state(
                |ui, window: &mut Window| {
                    super::summary_window(ui, &window.texts, (1, &window.summary));
                },
                window,
            );
        harness.run_steps(3);
        assert_layout(&harness, &note)?;
        let image = harness.render()?;
        save(
            &format!("model-material-note-{}-{tag}.png", case.tag()),
            image.width(),
            image.height(),
            &image.into_raw(),
        )?;
    }
    Ok(())
}

fn fixture(language: Language, case: Case) -> Result<Window, Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let archive_path = folder.path().join("fixture.p4k");
    let mut entries = testkit::material_reference_entries(REFERENCE, None);
    if matches!(case, Case::MappedUnreadable) {
        entries.push(TestEntry::stored(TARGET, b"not an XML material".to_vec()));
    }
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&archive_path)?;
    let archive = Archive::open(&archive_path)?;
    let source = ModelSource::from_archive(&archive, testkit::MATERIAL_REFERENCE_MODEL)?;
    let written = source.write_glb(&GlbOptions::default(), &folder.path().join("fixture.glb"))?;
    let summary = ModelExportSummary::new(
        PathBuf::from("fixture.glb"),
        0,
        source.model(),
        (written, &AnimationSet::default()),
        false,
    );
    Ok(Window {
        texts: Texts::new(language),
        summary,
    })
}

fn assert_layout(harness: &Harness<'_, Window>, note: &str) -> TestResult {
    use egui_kittest::kittest::{NodeT, Queryable};

    let bounds = harness
        .get_by_label(note)
        .accesskit_node()
        .bounding_box()
        .ok_or("material note bounds missing")?;
    let close = harness.state().texts.get(ExtractionText::Close);
    let footer = harness
        .get_by_role_and_label(Role::Button, &close)
        .accesskit_node()
        .bounding_box()
        .ok_or("close action bounds missing")?;
    let viewport = placement::dialog_area(&harness.ctx);
    assert!(
        bounds.y1 - bounds.y0 > f64::from(ROW_HEIGHT),
        "the complete long material note wraps onto multiple lines"
    );
    assert!(
        bounds.x0 >= f64::from(viewport.left())
            && bounds.x1 <= f64::from(viewport.right())
            && bounds.y0 >= f64::from(viewport.top())
            && bounds.y1 <= footer.y0,
        "the complete material note remains visible above the footer"
    );
    assert!(
        footer.y1 - footer.y0 >= f64::from(BUTTON_HEIGHT)
            && footer.y1 <= f64::from(viewport.bottom()),
        "the close action remains fully visible"
    );
    Ok(())
}

fn save(name: &str, width: u32, height: u32, image: &[u8]) -> TestResult {
    let Some(folder) = std::env::var_os("P4KX_RENDER_DIR").map(PathBuf::from) else {
        return Ok(());
    };
    fs::create_dir_all(&folder)?;
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
