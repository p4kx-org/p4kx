// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::DataForge;
use p4kx_extraction::gfx::{Background, CanvasSize, RasterOptions};
use p4kx_screens::{ContentOmissionCode, Still, StillFormat, StillRequest, WidgetSection};
use p4kx_testkit::{
    DataForgeBuilder, TestCanvas, TestDim, TestValue, TestWidget, entity_schema, screens_schema,
};

use super::UiReport;
use crate::UiText;

const CANVAS: u128 = 0x5a00_0000_0000_4000_8000_0000_0000_0095;
const CANVAS_PATH: &str = "libs/foundry/records/ui/buildingblocks/test/omissions";

fn render_omissions() -> Result<Still, Box<dyn Error>> {
    let picture = |name| {
        TestWidget::new("BuildingBlocks_WidgetRuntimeImage", name)
            .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
    };
    let canvas = TestCanvas::new(CANVAS, "omissions", 60.0, 20.0)
        .widget(picture("map").with("imageSource", TestValue::text("synthetic-map")))
        .widget(picture("dock").with("imageSource", TestValue::text("synthetic-dock")))
        .widget(picture("display").with("imageSource", TestValue::text("")));
    let bytes = screens_schema(entity_schema(DataForgeBuilder::new()))
        .record(canvas.record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let request = StillRequest {
        screen: CANVAS_PATH,
        brand: None,
        variables: &[],
        raster: RasterOptions {
            size: CanvasSize::Stage,
            background: Background::Transparent,
        },
        format: StillFormat::Png,
    };
    Ok(Still::render(&forge, None, None, &request)?)
}

#[test]
fn the_ui_report_preserves_each_authored_omission_and_the_group_counts()
-> Result<(), Box<dyn Error>> {
    let still = render_omissions()?;
    let report = UiReport::of_still(&still, &[]);
    assert_eq!(
        report.content_omissions(),
        still.report().content_omissions.as_slice(),
        "the projection carries every original field"
    );
    let source_rows: Vec<_> = report
        .content_omissions()
        .iter()
        .map(|row| {
            (
                row.widget.as_str(),
                row.widget_input.section,
                row.widget_input.slot,
                row.field.as_str(),
                row.source.as_deref(),
                row.code,
            )
        })
        .collect();
    assert_eq!(
        source_rows,
        [
            (
                "map",
                WidgetSection::Scene,
                0,
                "imageSource",
                Some("synthetic-map"),
                ContentOmissionCode::RuntimeTarget
            ),
            (
                "dock",
                WidgetSection::Scene,
                1,
                "imageSource",
                Some("synthetic-dock"),
                ContentOmissionCode::RuntimeTarget
            ),
            (
                "display",
                WidgetSection::Scene,
                2,
                "imageSource",
                None,
                ContentOmissionCode::SourceMissing
            ),
        ],
        "widget names, original positions, fields, sources and omission reasons"
    );
    assert_eq!(report.len(), 2, "left-out count counts grouped lines");
    assert!(
        report
            .lines()
            .iter()
            .all(|row| row.label == UiText::ReportLeftOut),
        "existing diagnostics stay grouped"
    );
    Ok(())
}
