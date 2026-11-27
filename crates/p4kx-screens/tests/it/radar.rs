// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic radar layout: the frame of the map area, rings, spokes, a standard separator and
//! the widgets the game fills at run time, against a stored reference image.
//!
//! Set `P4KX_UPDATE_REFERENCE` to write the reference again after a deliberate change.

use p4kx_dataforge::DataForge;
use p4kx_screens::{Composed, ContentOmissionCode, ImageSources, ScreenComposer};
use p4kx_testkit::{
    TestCanvas, TestColour, TestCondition, TestDim, TestModifier, TestRecord, TestStyleEntry,
    TestValue, TestWidget, named_tag_record, tag_record,
};

use crate::images::{texture, vector};
use crate::support::{SCREEN, TestResult, database, guid_text, matches_reference, pixel, stage};

const REFERENCE: &str = "tests/it/reference/radar_sample.png";
const KIT_FOLDER: &str = "libs/foundry/records/ui/buildingblocks/modularkit/standard/widgets/";
const SEPARATOR_CANVAS: u128 = 0x5a00_0000_0000_4000_8000_0000_0000_0020;
const ROOT_TAG: u128 = 0x5a00_0000_0000_4000_8000_0000_0000_0021;
const HORIZONTAL_TAG: u128 = 0x5a00_0000_0000_4000_8000_0000_0000_0022;
const SECONDARY_TAG: u128 = 0x5a00_0000_0000_4000_8000_0000_0000_0023;
const CYAN: TestColour = TestColour::Solid([0, 200, 200, 255]);
const DIM_CYAN: TestColour = TestColour::Solid([0, 200, 200, 90]);
const AMBER: [u8; 4] = [255, 170, 0, 255];
const NIGHT: TestColour = TestColour::Solid([8, 12, 20, 255]);

fn circle(name: &str, side: f32) -> TestWidget {
    TestWidget::new("BuildingBlocks_WidgetCircle", name)
        .parent("frame")
        .size(TestDim::Fixed(side), TestDim::Fixed(side))
        .anchor(0.5, 0.5)
        .pivot(0.5, 0.5)
        .with("doFill", TestValue::Bool(false))
        .with("doStroke", TestValue::Bool(true))
        .with("strokeWidth", TestValue::Single(2.0))
        .with("strokeColor", CYAN.value())
}

fn spoke(name: &str, degrees: f32) -> TestWidget {
    TestWidget::display(name)
        .parent("frame")
        .size(TestDim::Fixed(1.0), TestDim::Fixed(80.0))
        .anchor(0.5, 0.5)
        .pivot(0.5, 0.5)
        .rotate(degrees)
        .background(DIM_CYAN)
}

fn run_time(kind: &'static str, name: &str, side: f32, offset: [f32; 2]) -> TestWidget {
    TestWidget::new(kind, name)
        .size(TestDim::Fixed(side), TestDim::Fixed(side))
        .offset(offset[0], offset[1])
}

fn radar() -> TestCanvas {
    TestCanvas::new(SCREEN, "radar", 160.0, 130.0)
        .raw()
        .widget(TestWidget::display("backdrop").background(NIGHT))
        .widget(
            TestWidget::display("frame")
                .size(TestDim::Fixed(100.0), TestDim::Fixed(100.0))
                .offset(10.0, 10.0)
                .border(2.0, CYAN),
        )
        .widget(
            TestWidget::new("BuildingBlocks_WidgetRuntimeImage", "map")
                .parent("frame")
                .with("imageSource", TestValue::text("InteriorMap")),
        )
        .widget(circle("ring_outer", 80.0))
        .widget(circle("ring_inner", 40.0))
        .widget(spoke("spoke_000", 0.0))
        .widget(spoke("spoke_090", 90.0))
        .widget(spoke("spoke_045", 45.0))
        .widget(
            TestWidget::new("BuildingBlocks_WidgetSeparator", "range_rule")
                .size(TestDim::Fixed(100.0), TestDim::Fixed(6.0))
                .offset(10.0, 116.0)
                .with("direction", TestValue::text("Horizontal"))
                .with("style", TestValue::text("Secondary")),
        )
        .widget(run_time(
            "BuildingBlocks_WidgetLine",
            "stalk",
            4.0,
            [120.0, 10.0],
        ))
        .widget(run_time(
            "BuildingBlocks_GeometryEntityWidget",
            "marker_3d",
            12.0,
            [120.0, 30.0],
        ))
        .widget(run_time(
            "BuildingBlocks_WidgetWindow",
            "view",
            12.0,
            [120.0, 50.0],
        ))
        .widget(run_time(
            "BuildingBlocks_WidgetLight",
            "sun",
            12.0,
            [120.0, 70.0],
        ))
        .widget(run_time(
            "BuildingBlocks_WidgetEditBox",
            "search",
            12.0,
            [120.0, 90.0],
        ))
}

fn separator_canvas() -> TestCanvas {
    let look = TestStyleEntry::new("root")
        .when(TestCondition::Tag(ROOT_TAG))
        .when(TestCondition::Parent(vec![
            TestCondition::Tag(HORIZONTAL_TAG),
            TestCondition::Tag(SECONDARY_TAG),
        ]))
        .set(TestModifier::EnableBackground(true))
        .set(TestModifier::Background(TestColour::Solid(AMBER)))
        .set(TestModifier::Number("MinSizeY", 2.0))
        .set(TestModifier::Number("MaxSizeY", 2.0))
        .set(TestModifier::Number("PivotY", 0.5))
        .set(TestModifier::Number("AnchorY", 0.5));
    TestCanvas::new(
        SEPARATOR_CANVAS,
        "horizontalseparatorsecondarywidgetstandard",
        1920.0,
        1080.0,
    )
    .raw()
    .in_folder(KIT_FOLDER)
    .embedded(look)
    .widget(TestWidget::display("ComponentRoot").tag(ROOT_TAG))
}

fn others() -> Vec<TestRecord> {
    vec![
        separator_canvas().record(),
        tag_record(ROOT_TAG, "Root"),
        named_tag_record(
            HORIZONTAL_TAG,
            "UI.Style._Standard.ModularKit.Widgets.separator.horizontal",
        ),
        named_tag_record(
            SECONDARY_TAG,
            "UI.Style._Standard.ModularKit.Widgets.separator.secondary",
        ),
    ]
}

fn compose(
    canvas: &TestCanvas,
    others: Vec<TestRecord>,
) -> Result<Composed, Box<dyn std::error::Error>> {
    let bytes = database(std::iter::once(canvas.record()).chain(others));
    let forge = DataForge::open(&bytes)?;
    Ok(ScreenComposer::new(&forge).compose(&guid_text(SCREEN), None)?)
}

#[test]
fn the_radar_sample_matches_its_reference_image() -> TestResult {
    let image = compose(&radar(), others())?.rasterize(&stage())?.image;
    matches_reference(REFERENCE, &image, 3, 12)
}

#[test]
fn the_frame_the_rings_and_the_spokes_are_drawn() -> TestResult {
    let image = compose(&radar(), others())?.rasterize(&stage())?.image;
    let cyan = |at: [u8; 4]| at[2] > 150 && at[3] == 255;
    assert!(cyan(pixel(&image, 10, 60)), "the left edge of the frame");
    assert!(cyan(pixel(&image, 60, 10)), "the top edge of the frame");
    assert!(cyan(pixel(&image, 99, 60)), "the outer ring");
    assert!(cyan(pixel(&image, 79, 60)), "the inner ring");
    assert_ne!(pixel(&image, 60, 20), pixel(&image, 61, 20), "the spoke");
    Ok(())
}

#[test]
fn a_separator_takes_its_look_from_the_standard_canvas_and_the_tags_the_game_gives_it() -> TestResult
{
    let image = compose(&radar(), others())?.rasterize(&stage())?.image;
    let rows: Vec<u32> = (112..126)
        .filter(|&y| pixel(&image, 60, y) == AMBER)
        .collect();
    assert_eq!(rows, [118, 119], "two rows, centred in the box of 6 rows");
    assert_eq!(pixel(&image, 5, 118), [8, 12, 20, 255], "left of the box");
    Ok(())
}

#[test]
fn a_separator_without_its_standard_canvas_is_listed() -> TestResult {
    let composed = compose(&radar(), Vec::new())?;
    let note = composed
        .report()
        .unsupported
        .iter()
        .find(|entry| entry.element == "WidgetSeparator")
        .ok_or("the separator is not listed")?;
    assert!(note.reason.contains("no canvas"), "{}", note.reason);
    Ok(())
}

#[test]
fn what_the_game_fills_at_run_time_is_listed_with_its_reason() -> TestResult {
    let composed = compose(&radar(), others())?;
    let report = composed.report();
    let reason_of = |element: &str| {
        report
            .unsupported
            .iter()
            .find(|entry| entry.element == element)
            .map(|entry| entry.reason.as_str())
    };
    assert!(
        reason_of("WidgetRuntimeImage").is_some_and(|reason| reason.contains("run time")),
        "the map"
    );
    assert!(
        reason_of("GeometryEntityWidget").is_some_and(|reason| reason.contains("3D")),
        "the 3D marker"
    );
    assert!(
        reason_of("WidgetWindow").is_some_and(|reason| reason.contains("3D")),
        "the window"
    );
    assert!(
        reason_of("WidgetLine").is_some_and(|reason| reason.contains("run time")),
        "the stalk line"
    );
    assert_eq!(
        reason_of("WidgetLight"),
        None,
        "a light has nothing to draw"
    );
    assert_eq!(
        reason_of("WidgetEditBox"),
        None,
        "an edit box draws its text widget"
    );
    assert_eq!(
        report.images_left_out, 3,
        "the map, the marker and the window"
    );
    assert!(
        report
            .unsupported
            .iter()
            .all(|entry| !entry.reason.contains("no renderer")),
        "no widget is left without a renderer"
    );
    Ok(())
}

#[test]
fn a_shape_without_a_graphic_is_the_box_its_style_draws() -> TestResult {
    let canvas = TestCanvas::new(SCREEN, "plates", 40.0, 10.0)
        .raw()
        .widget(TestWidget::display("row").flex("Row", "Start", "Start", 0.0))
        .widget(
            TestWidget::new("BuildingBlocks_WidgetCustomShape", "plate")
                .parent("row")
                .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
                .with("svgPath", TestValue::text(""))
                .background(CYAN),
        )
        .widget(
            TestWidget::new("BuildingBlocks_WidgetCustomShape", "ghost")
                .parent("row")
                .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
                .with("svgPath", TestValue::text("")),
        );
    let bytes = database([canvas.record()]);
    let forge = DataForge::open(&bytes)?;
    let composed = ScreenComposer::new(&forge)
        .with_images(ImageSources {
            texture: &texture,
            vector: &vector,
        })
        .compose(&guid_text(SCREEN), None)?;
    let image = composed.rasterize(&stage())?.image;
    assert_eq!(
        pixel(&image, 5, 5),
        [0, 200, 200, 255],
        "the plate is drawn"
    );
    let report = composed.report();
    let bare = report
        .unsupported
        .iter()
        .find(|entry| entry.reason.contains("names no vector graphic"))
        .ok_or("the shape without a box is not listed")?;
    assert_eq!(bare.examples, ["ghost"], "the shape that draws nothing");
    let boxed = report
        .unsupported
        .iter()
        .find(|entry| entry.reason.contains("the box is drawn"))
        .ok_or("the shape with a box is not listed")?;
    assert_eq!(boxed.examples, ["plate"], "the shape whose box draws");
    assert_eq!(report.images_left_out, 2);
    assert!(
        bare.reason.contains("svgPath") && boxed.reason.contains("svgPath"),
        "the field that would name the graphic: {} / {}",
        bare.reason,
        boxed.reason
    );
    Ok(())
}

#[test]
fn a_run_time_image_is_listed_with_the_field_that_names_it() -> TestResult {
    let composed = compose(&radar(), others())?;
    let entry = composed
        .report()
        .unsupported
        .iter()
        .find(|entry| entry.element == "WidgetRuntimeImage")
        .ok_or("the map is not listed")?;
    assert!(entry.reason.contains("imageSource"), "{}", entry.reason);
    let missing = composed
        .report()
        .content_omissions
        .iter()
        .find(|entry| entry.element == "WidgetRuntimeImage")
        .ok_or("no render target omission")?;
    assert_eq!(missing.widget, "map");
    assert_eq!(missing.field, "imageSource");
    assert_eq!(missing.source.as_deref(), Some("InteriorMap"));
    assert_eq!(missing.code, ContentOmissionCode::RuntimeTarget);
    Ok(())
}

#[test]
fn widget_names_are_listed_as_the_records_hold_them() -> TestResult {
    let canvas = TestCanvas::new(SCREEN, "names", 20.0, 20.0).raw().widget(
        TestWidget::new("BuildingBlocks_WidgetRuntimeImage", "<unnamed> & \"co\"")
            .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
            .with("imageSource", TestValue::text("")),
    );
    let composed = compose(&canvas, Vec::new())?;
    let entry = composed
        .report()
        .unsupported
        .iter()
        .find(|entry| entry.element == "WidgetRuntimeImage")
        .ok_or("the widget is not listed")?;
    assert_eq!(entry.examples, ["<unnamed> & \"co\""]);
    let missing = composed
        .report()
        .content_omissions
        .first()
        .ok_or("no missing render target omission")?;
    assert_eq!(missing.widget, "<unnamed> & \"co\"");
    assert_eq!(missing.source, None);
    assert_eq!(missing.code, ContentOmissionCode::SourceMissing);
    Ok(())
}
