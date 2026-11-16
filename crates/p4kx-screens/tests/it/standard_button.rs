// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_screens::{Composed, ScreenComposer, TextSources};
use p4kx_testkit::{
    TestCanvas, TestColour, TestCondition, TestDim, TestInstance, TestModifier, TestRecord,
    TestStyleEntry, TestValue, TestWidget, named_tag_record, screens_text_sample,
};

use crate::support::{
    BLUE, GREEN, RED, SCREEN, TestResult, blue_at, empty_at, green_at, guid_text, red_at, stage,
};
use crate::text::fonts;
use crate::text_part::{field, field_entry};

#[path = "standard_button_general.rs"]
mod general;
#[path = "standard_button_vector.rs"]
mod vector;

const COMPONENT: u128 = SCREEN + 1;
const FILLED: u128 = SCREEN + 2;
const GHOST: u128 = SCREEN + 3;
const LEFT: u128 = SCREEN + 4;
const TOP: u128 = SCREEN + 5;
const FRAME: u128 = SCREEN + 6;
const BUTTON: &str = "BuildingBlocks_ComponentGeneralButtonSecondary";
const FOLDER: &str = "libs/foundry/records/ui/buildingblocks/modularkit/standard/components/";

fn button(fill: &str, position: &str, show_parts: bool) -> TestRecord {
    component(BUTTON, fill, position, show_parts)
}

fn component(kind: &'static str, fill: &str, position: &str, show_parts: bool) -> TestRecord {
    let label = TestInstance::new("BuildingBlocks_ComponentLabelProperties")
        .value("show", TestValue::Bool(show_parts))
        .value("label", TestValue::text("@fixture_button_label"))
        .value("scale", TestValue::Single(1.0));
    let icon = TestInstance::new("BuildingBlocks_ComponentIconProperties")
        .value("show", TestValue::Bool(show_parts))
        .value("scale", TestValue::Single(6.0));
    TestCanvas::new(SCREEN, "buttons", 40.0, 20.0)
        .widget(
            TestWidget::new(kind, "button")
                .size(TestDim::Fixed(32.0), TestDim::Fixed(20.0))
                .with("sizingMethod", TestValue::text("Size"))
                .with("fillStyle", TestValue::text(fill))
                .with("iconPosition", TestValue::text(position))
                .with("labelProperties", TestValue::Struct(label))
                .with("iconProperties", TestValue::Struct(icon)),
        )
        .record()
}

fn frame_style(name: &str, fill: u128, position: u128, colour: TestColour) -> TestStyleEntry {
    TestStyleEntry::new(name)
        .when(TestCondition::Tag(FRAME))
        .when(TestCondition::Parent(vec![
            TestCondition::Tag(fill),
            TestCondition::Tag(position),
        ]))
        .set(TestModifier::EnableBackground(true))
        .set(TestModifier::Background(colour))
}

fn template(proxy: &str) -> TestRecord {
    TestCanvas::new(
        COMPONENT,
        "buttongeneralsecondarycomponentstandard",
        32.0,
        20.0,
    )
    .in_folder(FOLDER)
    .raw()
    .embedded(frame_style("filled at left", FILLED, LEFT, BLUE))
    .embedded(frame_style("ghost at top", GHOST, TOP, GREEN))
    .embedded(field_entry())
    .widget(TestWidget::display("root"))
    .widget(
        TestWidget::display("icon")
            .parent("root")
            .size(TestDim::Fixed(20.0), TestDim::Fixed(4.0))
            .offset(1.0, 1.0)
            .background(RED)
            .with("layer", TestValue::Int32(7)),
    )
    .widget(TestWidget::display("frame").parent("root").tag(FRAME))
    .widget(field("label", 12.0).with("layer", TestValue::Int32(8)))
    .widget(
        TestWidget::display("other root")
            .size(TestDim::Fixed(4.0), TestDim::Fixed(4.0))
            .offset(36.0, 0.0)
            .background(RED),
    )
    .record()
    .value("proxyRootNode", TestValue::pointer(proxy))
    .value("operations", template_operations())
}

fn operation(kind: &str, name: &str) -> TestInstance {
    TestInstance::new(format!("BuildingBlocks_Bindings{kind}")).named(name)
}

fn parameter(kind: &str, name: &str, parameter: &str, value: TestValue) -> TestInstance {
    operation(&format!("{kind}ComponentParameter"), name)
        .value("parameter", TestValue::text(parameter))
        .value("defaultValue", value)
}

fn bind(kind: &str, widget: &str, field: &str, input: &str) -> TestInstance {
    TestInstance::new(format!("BuildingBlocks_Bindings{kind}Field"))
        .value("widget", TestValue::pointer(widget))
        .value("field", TestValue::text(field))
        .value("input", TestValue::pointer(input))
}

fn tag_switch(name: &str, input: &str, values: [(i64, u128); 2], default: u128) -> TestInstance {
    let cases = values.into_iter().map(|(key, tag)| {
        TestValue::Struct(
            TestInstance::new("BuildingBlocks_IntegerTagPair")
                .value("first", TestValue::Int64(key))
                .value("second", TestValue::Reference(tag)),
        )
    });
    operation("TagFromIntegerSwitch", name)
        .value("input", TestValue::pointer(input))
        .value("values", TestValue::Array(cases.collect()))
        .value("defaultValue", TestValue::Reference(default))
}

fn template_operations() -> TestValue {
    let operations = [
        parameter("Integer", "fill", "ParamInput3", TestValue::Int64(111)),
        tag_switch("fill tag", "fill", [(432, FILLED), (111, GHOST)], GHOST),
        bind("String", "root", "PrimaryStateTag", "fill tag"),
        parameter("Integer", "position", "ParamInput4", TestValue::Int64(4)),
        tag_switch("position tag", "position", [(4, TOP), (82, LEFT)], TOP),
        bind("String", "root", "SecondaryStateTag", "position tag"),
        parameter(
            "Boolean",
            "show label",
            "ParamInput0",
            TestValue::Bool(true),
        ),
        bind("Boolean", "label", "IsActive", "show label"),
        parameter("Boolean", "show icon", "ParamInput1", TestValue::Bool(true)),
        bind("Boolean", "icon", "IsActive", "show icon"),
        parameter(
            "Localized",
            "label text",
            "ParamInput0",
            TestValue::text("@fixture_template_text"),
        ),
        bind("Localized", "label", "ParamInput0", "label text"),
        parameter(
            "Number",
            "icon width",
            "ParamInput4",
            TestValue::Single(21.0),
        ),
        bind("Number", "icon", "SizeX", "icon width"),
    ];
    TestValue::Array(operations.into_iter().map(TestValue::Struct).collect())
}

fn compose(
    button: TestRecord,
    proxy: &str,
    saw_label: &Cell<bool>,
) -> Result<Composed, Box<dyn Error>> {
    let bytes = screens_text_sample()
        .record(button)
        .record(template(proxy))
        .record(named_tag_record(FILLED, "FillStyle_Filled"))
        .record(named_tag_record(GHOST, "FillStyle_Ghost"))
        .record(named_tag_record(LEFT, "IconPos_Left"))
        .record(named_tag_record(TOP, "IconPos_Top"))
        .record(named_tag_record(FRAME, "Frame"))
        .build();
    let forge = DataForge::open(&bytes)?;
    let fonts = fonts()?;
    let localise = |key: &str| {
        if key == "fixture_button_label" {
            saw_label.set(true);
            Some("A".to_owned())
        } else {
            None
        }
    };
    Ok(ScreenComposer::new(&forge)
        .with_texts(TextSources {
            fonts: &fonts,
            localise: Some(&localise),
        })
        .compose(&guid_text(SCREEN), None)?)
}

#[test]
fn secondary_button_draws_its_proxy_styled_frame_and_bound_label() -> TestResult {
    let saw_label = Cell::new(false);
    let composed = compose(button("Filled", "Left", true), "root", &saw_label)?;
    let image = composed.rasterize(&stage())?.image;
    assert!(
        blue_at(&image, 29, 17),
        "both selected state tags style the frame"
    );
    assert!(
        red_at(&image, 3, 2),
        "the icon paints above the frame by layer"
    );
    assert!(
        blue_at(&image, 9, 2),
        "the Number parameter is independent of the Integer position"
    );
    assert!(
        empty_at(&image, 37, 2),
        "the other template root is not mounted"
    );
    assert!(
        saw_label.get(),
        "the instance's localization key reaches the text field"
    );
    assert_eq!(composed.report().texts_drawn, 1);
    Ok(())
}

#[test]
fn secondary_button_hides_parts_when_the_instance_turns_them_off() -> TestResult {
    let saw_label = Cell::new(false);
    let composed = compose(button("Ghost", "Top", false), "root", &saw_label)?;
    let image = composed.rasterize(&stage())?.image;
    assert!(
        green_at(&image, 29, 17),
        "the alternative state draws its frame"
    );
    assert!(green_at(&image, 3, 2), "the inherited icon is hidden");
    assert!(!saw_label.get(), "the hidden label has no localization use");
    assert_eq!(composed.report().texts_drawn, 0);
    Ok(())
}

#[test]
fn secondary_button_with_an_invalid_proxy_reports_it_without_mounting_the_template() -> TestResult {
    let composed = compose(button("Filled", "Left", false), "frame", &Cell::new(false))?;
    assert_eq!(
        composed.report().canvases,
        1,
        "only the outer canvas was admitted"
    );
    assert!(
        composed.report().unsupported.iter().any(|entry| {
            entry.reason == "the button template has no valid proxy root"
                && entry.examples == ["button"]
        }),
        "the diagnostic names the owning button"
    );
    Ok(())
}
