// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::PathBuf;

use eframe::egui_wgpu::RendererOptions;
use egui::ScrollArea;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_extraction::DataForge;
use p4kx_extraction::gfx::{Background, CanvasSize, RasterOptions};
use p4kx_screens::{ContentOmission, Still, StillFormat, StillRequest};
use p4kx_testkit::{
    DataForgeBuilder, TestCanvas, TestDim, TestValue, TestWidget, entity_schema, screens_schema,
};

use crate::ui::software_wgpu::software_wgpu_setup;
use crate::{Language, Texts, UiText};

const ROWS: usize = 48;
const VIEWPORT: [f32; 2] = [640.0, 400.0];
const SCREEN: &str = "libs/foundry/records/ui/buildingblocks/test/scroll_omissions";
const FIRST_WIDGET: &str = "the-first-widget";
const LAST_WIDGET: &str = "the-last-widget";

struct Report {
    texts: Texts,
    rows: Vec<ContentOmission>,
    scroll_to_end: bool,
}

fn render_omissions() -> Result<Vec<ContentOmission>, Box<dyn Error>> {
    let canvas = (0..ROWS).fold(
        TestCanvas::new(1, "scroll_omissions", 80.0, 40.0),
        |canvas, index| {
            let (name, source) = match index {
                0 => (FIRST_WIDGET.to_owned(), "fixture-target"),
                index if index == ROWS - 1 => (LAST_WIDGET.to_owned(), ""),
                index => (format!("widget-{index}"), "fixture-target"),
            };
            canvas.widget(
                TestWidget::new("BuildingBlocks_WidgetRuntimeImage", &name)
                    .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
                    .with("imageSource", TestValue::text(source)),
            )
        },
    );
    let bytes = screens_schema(entity_schema(DataForgeBuilder::new()))
        .record(canvas.record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let request = StillRequest {
        screen: SCREEN,
        brand: None,
        variables: &[],
        raster: RasterOptions {
            size: CanvasSize::Stage,
            background: Background::Transparent,
        },
        format: StillFormat::Png,
    };
    let still = Still::render(&forge, None, None, &request)?;
    Ok(still.report().content_omissions.clone())
}

fn show_report(ui: &mut egui::Ui, state: &mut Report) {
    let mut scroll = ScrollArea::vertical().auto_shrink(false);
    if std::mem::take(&mut state.scroll_to_end) {
        scroll = scroll.vertical_scroll_offset(super::row_offset(state.rows.len()));
    }
    scroll.show(ui, |ui| super::show(ui, &state.texts, &state.rows));
}

#[test]
fn scrolling_the_report_keeps_original_omissions_in_both_languages() -> Result<(), Box<dyn Error>> {
    let rows = render_omissions()?;
    assert_eq!(
        rows.len(),
        ROWS,
        "all original widget slots reach the report"
    );
    let folder = std::env::var_os("P4KX_RENDER_DIR").map(PathBuf::from);
    for (language, tag) in [
        (Language::BritishEnglish, "en-GB"),
        (Language::German, "de-DE"),
    ] {
        check_report(language, tag, &rows, folder.as_deref())?;
    }
    Ok(())
}

fn check_report(
    language: Language,
    tag: &str,
    rows: &[ContentOmission],
    folder: Option<&std::path::Path>,
) -> Result<(), Box<dyn Error>> {
    let texts = Texts::new(language);
    let mut builder = Harness::builder()
        .with_context_setup(crate::theme::apply)
        .with_size(VIEWPORT)
        .with_max_steps(16);
    if folder.is_some() {
        builder = builder.renderer(WgpuTestRenderer::from_render_state(create_render_state(
            software_wgpu_setup(),
            RendererOptions::PREDICTABLE,
        )));
    }
    let mut harness = builder.build_ui_state(
        show_report,
        Report {
            texts,
            rows: rows.to_vec(),
            scroll_to_end: false,
        },
    );
    harness.run_steps(3);
    harness.get_by_label_contains(FIRST_WIDGET);
    assert!(
        harness.query_by_label_contains(LAST_WIDGET).is_none(),
        "off-screen widgets are virtualised in {tag}"
    );
    harness.state_mut().scroll_to_end = true;
    harness.run();
    let last = harness.get_by_label_contains(LAST_WIDGET);
    let expected = harness.state().texts.get(UiText::ReportOmissionNoSource);
    let bounds = last.rect();
    assert!(
        bounds.center().y >= 0.0 && bounds.center().y <= VIEWPORT[1],
        "the final original widget is in the viewport in {tag}: {bounds:?}"
    );
    harness.get_by_label_contains(&expected);
    harness.ctx.style_mut_of(harness.ctx.theme(), |style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
    harness.event(egui::Event::PointerMoved(bounds.center()));
    harness.run();
    if let Some(folder) = folder {
        std::fs::create_dir_all(folder)?;
        harness
            .render()?
            .save(folder.join(format!("ui-content-omissions-{tag}.png")))?;
    }
    assert_eq!(
        harness.query_all_by_label_contains(&expected).count(),
        2,
        "the tooltip exposes the final row's clipped source detail in {tag} at {bounds:?}"
    );
    Ok(())
}
