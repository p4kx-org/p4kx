// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! What chooses and names the page of an MFD, and what the pages must not draw (ADR-0306).

use p4kx_dataforge::DataForge;
use p4kx_gfx::RgbaImage;
use p4kx_screens::{Composed, ScreenComposer, TextSources, Variable};
use p4kx_testkit::{
    TestCanvas, TestDim, TestInstance, TestOp, TestValue, TestWidget, screens_text_sample,
};

use crate::support::{
    RED, SCREEN, TestResult, database, empty_at, guid_text, near, pixel, red_at, stage,
};
use crate::text::fonts;
use crate::text_part::field_entry;

const PAGE: u128 = SCREEN + 0x400;
const CLEAR: [u8; 4] = [0, 0, 0, 0];

type Failure = Box<dyn std::error::Error>;

const fn square(widget: TestWidget) -> TestWidget {
    widget.size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
}

fn variables(settings: &[&str]) -> Result<Vec<Variable>, Failure> {
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

fn labelled_field(name: &'static str, label: &'static str) -> TestWidget {
    TestWidget::new("BuildingBlocks_WidgetTextField", name)
        .parent("root")
        .size(TestDim::Fixed(20.0), TestDim::Fixed(20.0))
        .with(
            "labelProperties",
            TestValue::Struct(
                TestInstance::new("BuildingBlocks_ComponentLabelProperties")
                    .value("label", TestValue::text(label)),
            ),
        )
}

fn text_of(canvas: &TestCanvas, settings: &[&str]) -> Result<RgbaImage, Failure> {
    let bytes = screens_text_sample().record(canvas.record()).build();
    let forge = DataForge::open(&bytes)?;
    let fonts = fonts()?;
    let composed = ScreenComposer::new(&forge)
        .with_variables(&variables(settings)?)
        .with_texts(TextSources {
            fonts: &fonts,
            localise: None,
        })
        .compose(&guid_text(SCREEN), None)?;
    Ok(composed.rasterize(&stage())?.image)
}

const EDITOR_LABEL: &str = "@ui_leaderboards_Loadout";

fn titled(label: &'static str, source: TestOp) -> TestCanvas {
    TestCanvas::new(SCREEN, "title", 20.0, 20.0)
        .embedded(field_entry())
        .operation("source", source)
        .widget(TestWidget::display("root"))
        .widget(labelled_field("title", label))
        .operation("bind", TestOp::LocalField("title", "ParamInput0", "source"))
}

#[test]
fn a_localised_variable_gives_the_text_a_field_draws_in_place_of_its_label() -> TestResult {
    let canvas = titled(EDITOR_LABEL, TestOp::NamedLocalVar("mfdviewname"));
    assert_eq!(
        pixel(&text_of(&canvas, &[])?, 2, 4),
        CLEAR,
        "the editor label of a field that is filled in shows nothing yet"
    );
    let named = text_of(&canvas, &["mfdviewname=A"])?;
    assert!(near(pixel(&named, 2, 4), [255, 0, 0, 255], 2), "the name");
    Ok(())
}

#[test]
fn the_default_of_a_localised_parameter_is_the_text_unless_it_is_a_placeholder() -> TestResult {
    let named = text_of(
        &titled(EDITOR_LABEL, TestOp::TextParam("ParamInput0", "A")),
        &[],
    )?;
    assert!(
        near(pixel(&named, 2, 4), [255, 0, 0, 255], 2),
        "the default"
    );
    let kept = text_of(
        &titled("A", TestOp::TextParam("ParamInput0", "@LOC_PLACEHOLDER")),
        &[],
    )?;
    assert!(
        near(pixel(&kept, 2, 4), [255, 0, 0, 255], 2),
        "a placeholder leaves a real label as it is"
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
