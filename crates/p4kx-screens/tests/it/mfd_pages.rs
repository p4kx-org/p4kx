// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! What chooses and names the page of an MFD, and what the pages must not draw (ADR-0306).

use p4kx_dataforge::DataForge;
use p4kx_gfx::RgbaImage;
use p4kx_screens::{Composed, ImageSources, ScreenComposer, Variable};
use p4kx_testkit::{
    TestCanvas, TestColour, TestCondition, TestDim, TestInstance, TestModifier, TestOp,
    TestStyleEntry, TestValue, TestWidget, tag_record,
};

use crate::images::{texture, vector};
use crate::support::{
    RED, SCREEN, TestResult, database, empty_at, guid_text, near, pixel, red_at, stage,
};

const PAGE: u128 = SCREEN + 0x400;

pub(crate) type Failure = Box<dyn std::error::Error>;

const fn square(widget: TestWidget) -> TestWidget {
    widget.size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
}

pub(crate) fn variables(settings: &[&str]) -> Result<Vec<Variable>, Failure> {
    settings
        .iter()
        .map(|setting| Ok(setting.parse::<Variable>()?))
        .collect()
}

fn compose(records: Vec<p4kx_testkit::TestRecord>, settings: &[&str]) -> Result<Composed, Failure> {
    let bytes = database(records);
    let forge = DataForge::open(&bytes)?;
    Ok(ScreenComposer::new(&forge)
        .with_variables(&variables(settings)?)
        .compose(&guid_text(SCREEN), None)?)
}

fn image_of(canvas: &TestCanvas, settings: &[&str]) -> Result<RgbaImage, Failure> {
    Ok(compose(vec![canvas.record()], settings)?
        .rasterize(&stage())?
        .image)
}

#[test]
fn the_constant_of_an_integer_comparison_is_read_whatever_its_width() -> TestResult {
    let screen = TestCanvas::new(SCREEN, "turret", 10.0, 10.0)
        .operation("mode", TestOp::IntVar)
        .operation("turret", TestOp::IntIs("mode", "Equal", 4))
        .widget(square(TestWidget::display("layout").background(RED)))
        .operation(
            "show",
            TestOp::BoolField("layout", "Instantiated", "turret"),
        );
    assert!(
        empty_at(&image_of(&screen, &[])?, 5, 5),
        "a mode of zero is no turret mode"
    );
    assert!(
        red_at(&image_of(&screen, &["variable=4"])?, 5, 5),
        "the mode 4 is"
    );
    Ok(())
}

const WIDE: i64 = 5_000_000_000;

#[test]
fn an_integer_constant_above_the_32_bit_range_is_read_exactly() -> TestResult {
    let screen = TestCanvas::new(SCREEN, "wide", 10.0, 10.0)
        .operation("mode", TestOp::IntVar)
        .operation("wide", TestOp::IntIs("mode", "Equal", WIDE))
        .widget(square(TestWidget::display("layout").background(RED)))
        .operation("show", TestOp::BoolField("layout", "Instantiated", "wide"));
    assert!(empty_at(&image_of(&screen, &["variable=4"])?, 5, 5));
    assert!(
        red_at(&image_of(&screen, &["variable=5000000000"])?, 5, 5),
        "the constant is not narrowed to 32 bits"
    );
    Ok(())
}

#[test]
fn an_integer_constant_beyond_exact_range_is_listed_and_not_read_as_zero() -> TestResult {
    let screen = TestCanvas::new(SCREEN, "huge", 10.0, 10.0)
        .operation("mode", TestOp::IntVar)
        .operation("huge", TestOp::IntIs("mode", "NotEqual", i64::MAX))
        .widget(square(TestWidget::display("layout").background(RED)))
        .operation("show", TestOp::BoolField("layout", "Instantiated", "huge"));
    let composed = compose(vec![screen.record()], &[])?;
    assert!(
        red_at(&composed.rasterize(&stage())?.image, 5, 5),
        "the widget keeps its own state, which a zero constant would have changed"
    );
    assert!(
        composed
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.element == "Binding" && entry.reason.contains("BooleanFromInteger")),
        "{:?}",
        composed.report().unsupported
    );
    Ok(())
}

#[test]
fn the_exceptions_of_an_integer_switch_are_read_in_every_width() -> TestResult {
    let screen = TestCanvas::new(SCREEN, "switch", 10.0, 10.0)
        .operation("mode", TestOp::IntVar)
        .operation("shown", TestOp::IntSwitch("mode", vec![4, WIDE]))
        .widget(square(TestWidget::display("layout").background(RED)))
        .operation("show", TestOp::BoolField("layout", "Instantiated", "shown"));
    assert!(
        red_at(&image_of(&screen, &[])?, 5, 5),
        "zero is no exception"
    );
    assert!(empty_at(&image_of(&screen, &["variable=4"])?, 5, 5));
    assert!(
        empty_at(&image_of(&screen, &["variable=5000000000"])?, 5, 5),
        "an exception above 32 bits is kept"
    );
    Ok(())
}

fn pager() -> TestCanvas {
    TestCanvas::new(SCREEN, "pager", 10.0, 10.0)
        .operation("page", TestOp::TextVar)
        .widget(square(TestWidget::new(
            "BuildingBlocks_WidgetCanvas",
            "holder",
        )))
        .operation("choose", TestOp::CanvasField("holder", "page"))
}

fn page() -> TestCanvas {
    TestCanvas::new(PAGE, "page_one", 10.0, 10.0)
        .widget(square(TestWidget::display("content").background(RED)))
}

#[test]
fn a_variable_that_names_a_canvas_puts_it_in_the_canvas_widget() -> TestResult {
    let by_guid = format!("variable={}", guid_text(PAGE));
    let by_path = "variable=libs/foundry/records/ui/buildingblocks/test/page_one".to_owned();
    for setting in [by_guid, by_path] {
        let composed = compose(vec![pager().record(), page().record()], &[&setting])?;
        assert!(
            red_at(&composed.rasterize(&stage())?.image, 5, 5),
            "{setting}"
        );
        assert_eq!(composed.report().unknown_variables().count(), 0);
    }
    let unset = compose(vec![pager().record(), page().record()], &[])?;
    assert!(empty_at(&unset.rasterize(&stage())?.image, 5, 5));
    Ok(())
}

#[test]
fn a_namespaced_variable_reaches_the_canvas_widgets_of_that_canvas_only() -> TestResult {
    let records = || vec![pager().record(), page().record()];
    let own = format!("pager::variable={}", guid_text(PAGE));
    let composed = compose(records(), &[&own])?;
    assert!(red_at(&composed.rasterize(&stage())?.image, 5, 5), "{own}");
    let other = format!("elsewhere::variable={}", guid_text(PAGE));
    let composed = compose(records(), &[&other])?;
    assert!(empty_at(&composed.rasterize(&stage())?.image, 5, 5));
    assert_eq!(composed.report().unknown_variables().count(), 1);
    Ok(())
}

#[test]
fn a_name_shared_by_two_canvases_is_listed_as_ambiguous() -> TestResult {
    let twin = TestCanvas::new(PAGE + 1, "page_one", 10.0, 10.0)
        .widget(square(TestWidget::display("content").background(RED)));
    let composed = compose(
        vec![pager().record(), page().record(), twin.record()],
        &["variable=libs/foundry/records/ui/buildingblocks/test/page_one"],
    )?;
    assert!(empty_at(&composed.rasterize(&stage())?.image, 5, 5));
    assert!(
        composed
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.reason.contains("CanvasReferenceRecord")
                && entry.reason.contains("2 canvas Records share")),
        "{:?}",
        composed.report().unsupported
    );
    Ok(())
}

#[test]
fn a_name_that_is_no_canvas_is_listed_and_draws_nothing() -> TestResult {
    let composed = compose(
        vec![pager().record(), page().record()],
        &["variable=no/such/page"],
    )?;
    assert!(empty_at(&composed.rasterize(&stage())?.image, 5, 5));
    assert!(
        composed
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.element == "Binding"
                && entry.reason.contains("CanvasReferenceRecord")
                && entry.reason.contains("no/such/page")),
        "{:?}",
        composed.report().unsupported
    );
    Ok(())
}

#[test]
fn a_runtime_image_has_no_background_but_a_box_has() -> TestResult {
    let screen = TestCanvas::new(SCREEN, "hologram", 20.0, 10.0)
        .widget(TestWidget::display("row").flex("Row", "Start", "Start", 0.0))
        .widget(square(
            TestWidget::new("BuildingBlocks_WidgetRuntimeImage", "picture")
                .parent("row")
                .background(RED),
        ))
        .widget(square(
            TestWidget::display("box").parent("row").background(RED),
        ));
    let image = image_of(&screen, &[])?;
    assert!(empty_at(&image, 5, 5), "the picture is the game's");
    assert!(red_at(&image, 15, 5), "a plain box keeps its fill");
    Ok(())
}

const INNER: u128 = SCREEN + 0x500;

#[test]
fn parameters_of_different_kinds_with_one_name_do_not_replace_each_other() -> TestResult {
    let inner = TestCanvas::new(INNER, "inner", 10.0, 10.0)
        .operation("shown", TestOp::BoolParam("ParamInput0", true))
        .widget(square(TestWidget::display("content").background(RED)))
        .operation("bind", TestOp::BoolField("content", "IsActive", "shown"));
    let outer = TestCanvas::new(SCREEN, "outer", 10.0, 10.0)
        .operation("flag", TestOp::BoolVar)
        .operation("amount", TestOp::NumVar)
        .widget(
            square(TestWidget::new("BuildingBlocks_WidgetCanvas", "holder"))
                .with("canvas", TestValue::Reference(INNER)),
        )
        .operation("hide", TestOp::BoolField("holder", "ParamInput0", "flag"))
        .operation("count", TestOp::NumField("holder", "ParamInput0", "amount"));
    let composed = compose(vec![outer.record(), inner.record()], &[])?;
    assert!(
        empty_at(&composed.rasterize(&stage())?.image, 5, 5),
        "the number does not take the place of the false boolean"
    );
    Ok(())
}

const RED_FILL: TestColour = RED;
const ARROW: u128 = SCREEN + 0x600;
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn icon(name: &'static str, at: f32) -> TestWidget {
    TestWidget::new("BuildingBlocks_WidgetIcon", name)
        .parent("root")
        .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
        .offset(at, 0.0)
        .with(
            "iconProperties",
            TestValue::Struct(
                TestInstance::new("BuildingBlocks_ComponentIconProperties")
                    .value("show", TestValue::Bool(true))
                    .value("iconPreset", TestValue::text("TestBoxX"))
                    .value("customIcon", TestValue::text("")),
            ),
        )
        .with(
            "svgFill",
            TestValue::Struct(
                TestInstance::new("BuildingBlocks_SvgFill")
                    .value("svgPath", TestValue::text(""))
                    .value("renderShape", TestValue::Bool(true))
                    .value("enableColorOverlay", TestValue::Bool(true)),
            ),
        )
}

#[test]
fn the_shape_of_an_icon_takes_the_colour_of_an_entry_that_selects_it_by_the_parent() -> TestResult {
    let arrow_colour = TestStyleEntry::new("arrow colour")
        .when(TestCondition::Type("CustomShape"))
        .when(TestCondition::Parent(vec![TestCondition::Tag(ARROW)]))
        .set(TestModifier::Fill(RED_FILL));
    let canvas = TestCanvas::new(SCREEN, "arrows", 30.0, 10.0)
        .embedded(arrow_colour)
        .widget(TestWidget::display("root"))
        .widget(icon("tagged", 0.0).tag(ARROW))
        .widget(icon("plain", 15.0));
    let bytes = database([canvas.record(), tag_record(ARROW, "Arrow")]);
    let forge = DataForge::open(&bytes)?;
    let composed = ScreenComposer::new(&forge)
        .with_images(ImageSources {
            texture: &texture,
            vector: &vector,
        })
        .compose(&guid_text(SCREEN), None)?;
    let image = composed.rasterize(&stage())?.image;
    assert!(red_at(&image, 5, 5), "the entry colours the shape");
    assert!(
        near(pixel(&image, 20, 5), WHITE, 2),
        "an icon without the tag keeps the colour of its graphic"
    );
    Ok(())
}
