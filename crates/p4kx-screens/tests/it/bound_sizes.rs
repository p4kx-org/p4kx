// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Sizes, anchors and the other number fields of the layout that a canvas binds to its widgets.

use p4kx_dataforge::DataForge;
use p4kx_screens::{Composed, ContentOmissionCode, ScreenComposer, Variable};
use p4kx_testkit::{
    TestCanvas, TestDim, TestInstance, TestModifier, TestOp, TestRecord, TestStyleEntry, TestValue,
    TestWidget, clone_modifiers,
};

use crate::support::{
    RED, SCREEN, TestResult, database, empty_at, guid_text, pixel, red_at, stage,
};

fn canvas() -> TestCanvas {
    TestCanvas::new(SCREEN, "bound_sizes", 40.0, 10.0)
}

fn bar() -> TestWidget {
    TestWidget::display("bar")
        .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
        .background(RED)
}

fn sized(canvas: TestCanvas, field: &'static str) -> TestCanvas {
    canvas
        .operation("flag", TestOp::BoolVar)
        .operation("size", TestOp::NumFromBool("flag", 30.0, 20.0))
        .widget(bar())
        .operation("bind", TestOp::NumField("bar", field, "size"))
}

fn compose(canvas: &TestCanvas, settings: &[&str]) -> Result<Composed, Box<dyn std::error::Error>> {
    compose_record(canvas.record(), settings)
}

fn compose_record(
    record: TestRecord,
    settings: &[&str],
) -> Result<Composed, Box<dyn std::error::Error>> {
    let bytes = database([record]);
    let forge = DataForge::open(&bytes)?;
    let variables = settings
        .iter()
        .map(|setting| Ok(setting.parse::<Variable>()?))
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    Ok(ScreenComposer::new(&forge)
        .with_variables(&variables)
        .compose(&guid_text(SCREEN), None)?)
}

#[test]
fn a_bound_size_sets_the_width_the_variables_decide() -> TestResult {
    let at_rest = compose(&sized(canvas(), "SizeX"), &[])?
        .rasterize(&stage())?
        .image;
    assert!(red_at(&at_rest, 15, 5), "the width is 20 at rest");
    assert!(empty_at(&at_rest, 25, 5));
    let set = compose(&sized(canvas(), "SizeX"), &["variable=true"])?
        .rasterize(&stage())?
        .image;
    assert!(red_at(&set, 25, 5), "the width is 30 with the variable");
    assert!(empty_at(&set, 35, 5));
    Ok(())
}

const WIDE_SWITCH_KEY: i64 = 1 << 40;

fn switch_case(key: i64, constant: f32, input: Option<&str>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("BuildingBlocks_IntegerNumberPair")
            .value("first", TestValue::Int64(key))
            .value("second", TestValue::Single(constant))
            .value(
                "secondOverride",
                input.map_or(TestValue::Null, TestValue::pointer),
            ),
    )
}

fn switch_record(default_override: Option<&str>) -> TestRecord {
    let selector = TestInstance::new("BuildingBlocks_BindingsIntegerVariable")
        .named("selector")
        .value("binding", TestValue::text("selector"));
    let number = TestInstance::new("BuildingBlocks_BindingsNumberComponentParameter")
        .named("number")
        .value("parameter", TestValue::text("override"))
        .value("defaultValue", TestValue::Single(32.0));
    let flag = TestInstance::new("BuildingBlocks_BindingsBooleanVariable")
        .named("flag")
        .value("binding", TestValue::text("flag"));
    let switch = TestInstance::new("BuildingBlocks_BindingsNumberFromIntegerSwitch")
        .named("switch")
        .value("input", TestValue::pointer("selector"))
        .value("defaultValue", TestValue::Single(14.0))
        .value(
            "defaultOverride",
            default_override.map_or(TestValue::Null, TestValue::pointer),
        )
        .value(
            "values",
            TestValue::Array(vec![
                switch_case(WIDE_SWITCH_KEY, 24.0, None),
                switch_case(WIDE_SWITCH_KEY, 4.0, Some("flag")),
                switch_case(2, 4.0, Some("number")),
                switch_case(3, 4.0, Some("flag")),
                switch_case(4, f32::NAN, None),
            ]),
        );
    let binding = TestInstance::new("BuildingBlocks_BindingsNumberField")
        .value("field", TestValue::text("SizeX"))
        .value("widget", TestValue::pointer("bar"))
        .value("input", TestValue::pointer("switch"));
    canvas().widget(bar()).record().value(
        "operations",
        TestValue::Array(
            [selector, number, flag, switch, binding]
                .into_iter()
                .map(TestValue::Struct)
                .collect(),
        ),
    )
}

#[test]
fn an_integer_number_switch_selects_the_first_exact_case_or_default() -> TestResult {
    let exact = format!("selector={WIDE_SWITCH_KEY}");
    let fractional = format!("selector={WIDE_SWITCH_KEY}.125");
    for (setting, default_override, width) in [
        (exact.as_str(), None, 24),
        (fractional.as_str(), None, 14),
        ("selector=2", None, 32),
        ("selector=5", Some("number"), 32),
    ] {
        let composed = compose_record(switch_record(default_override), &[setting])?;
        let image = composed.rasterize(&stage())?.image;
        assert!(red_at(&image, width - 2, 5), "{setting}");
        assert!(empty_at(&image, width + 2, 5), "{setting}");
        assert_eq!(
            composed.report().content_omissions,
            [],
            "valid case or fallback for {setting}"
        );
    }
    Ok(())
}

#[test]
fn an_integer_number_switch_diagnoses_an_invalid_selected_branch() -> TestResult {
    for (setting, default_override) in [
        ("selector=3", None),
        ("selector=4", None),
        ("selector=5", Some("flag")),
    ] {
        let composed = compose_record(switch_record(default_override), &[setting])?;
        let image = composed.rasterize(&stage())?.image;
        assert!(red_at(&image, 8, 5), "authored width for {setting}");
        assert!(empty_at(&image, 12, 5), "authored width for {setting}");
        let [omission] = composed.report().content_omissions.as_slice() else {
            return Err("one omission for the selected branch".into());
        };
        assert_eq!(omission.widget, "bar");
        assert_eq!(omission.widget_input.slot, 0);
        assert_eq!(omission.field, "SizeX");
        assert_eq!(omission.code, ContentOmissionCode::UnresolvedInput);
    }
    Ok(())
}

#[test]
fn a_style_entry_beats_a_bound_size() -> TestResult {
    let entry = TestStyleEntry::new("narrow").set(TestModifier::Number("SizeX", 5.0));
    let image = compose(&sized(canvas().embedded(entry), "SizeX"), &[])?
        .rasterize(&stage())?
        .image;
    assert!(red_at(&image, 2, 5));
    assert!(
        empty_at(&image, 8, 5),
        "the entry's 5 wins over the bound 20"
    );
    Ok(())
}

#[test]
fn a_size_that_cannot_be_decided_is_listed_and_the_widget_keeps_its_size() -> TestResult {
    let composed = compose(
        &canvas()
            .operation("mystery", TestOp::Other)
            .widget(bar())
            .operation("bind", TestOp::NumField("bar", "SizeX", "mystery")),
        &[],
    )?;
    let image = composed.rasterize(&stage())?.image;
    assert!(red_at(&image, 5, 5));
    assert!(empty_at(&image, 15, 5), "the width stays 10");
    assert!(
        composed
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.element == "Binding"
                && entry
                    .reason
                    .contains("the bound field SizeX is not evaluated")),
        "{:?}",
        composed.report().unsupported
    );
    Ok(())
}

#[test]
fn a_bound_depth_stays_listed_as_not_evaluated() -> TestResult {
    let composed = compose(&sized(canvas(), "PosZ"), &[])?;
    assert!(
        composed
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.element == "Binding" && entry.reason.contains("layout fields")),
        "{:?}",
        composed.report().unsupported
    );
    Ok(())
}

fn unbound_image() -> Result<p4kx_gfx::RgbaImage, Box<dyn std::error::Error>> {
    Ok(compose(&canvas().widget(bar()), &[])?
        .rasterize(&stage())?
        .image)
}

#[test]
fn every_kind_of_bound_layout_number_is_evaluated() -> TestResult {
    let unbound = unbound_image()?;
    let visible = [
        "AnchorX",
        "PivotX",
        "MarginLeft",
        "ScaleX",
        "BorderTopLeftRadius",
    ];
    let hidden = ["PaddingLeft", "FlexColumnSpacing"];
    for field in visible.into_iter().chain(hidden) {
        let composed = compose(&sized(canvas(), field), &[])?;
        let listed = composed
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.element == "Binding" && entry.reason.contains(field));
        assert!(!listed, "{field} is listed as not evaluated");
        let image = composed.rasterize(&stage())?.image;
        assert_eq!(
            image.pixels() != unbound.pixels(),
            visible.contains(&field),
            "{field}"
        );
    }
    Ok(())
}

#[test]
fn a_number_that_is_not_finite_or_out_of_range_keeps_the_authored_value() -> TestResult {
    let unbound = unbound_image()?;
    for (field, value) in [
        ("SizeX", f32::NAN),
        ("SizeX", f32::INFINITY),
        ("SizeX", 1.0e30),
        ("SizeX", -5.0),
        ("AnchorX", 1.0e30),
    ] {
        let composed = compose(
            &canvas()
                .operation("n", TestOp::NumParam("n", value))
                .widget(bar())
                .operation("bind", TestOp::NumField("bar", field, "n")),
            &[],
        )?;
        let image = composed.rasterize(&stage())?.image;
        assert_eq!(
            pixel(&image, 25, 5),
            pixel(&unbound, 25, 5),
            "{field} {value}"
        );
        assert_eq!(image.pixels(), unbound.pixels(), "{field} {value}");
        assert!(
            composed
                .report()
                .unsupported
                .iter()
                .any(|entry| entry.element == "Binding" && entry.reason.contains(field)),
            "{field} {value}: {:?}",
            composed.report().unsupported
        );
    }
    Ok(())
}

#[test]
fn a_value_that_is_not_a_number_is_listed_and_the_widget_keeps_its_size() -> TestResult {
    let composed = compose(
        &canvas()
            .operation("flag", TestOp::BoolVar)
            .widget(bar())
            .operation("bind", TestOp::NumField("bar", "SizeX", "flag")),
        &[],
    )?;
    let image = composed.rasterize(&stage())?.image;
    assert_eq!(image.pixels(), unbound_image()?.pixels());
    assert!(
        composed
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.reason.contains("a value that is not a number")),
        "{:?}",
        composed.report().unsupported
    );
    Ok(())
}

#[test]
fn a_modifier_of_a_clone_beats_the_size_bound_on_its_template() -> TestResult {
    let template = || {
        canvas()
            .operation("flag", TestOp::BoolVar)
            .operation("size", TestOp::NumFromBool("flag", 30.0, 20.0))
            .template(
                TestWidget::display("tmpl")
                    .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
                    .background(RED),
            )
            .operation("bind", TestOp::NumField("tmpl", "SizeX", "size"))
    };
    let plain = compose(
        &template().widget(TestWidget::clone_of("copy", "tmpl")),
        &[],
    )?
    .rasterize(&stage())?
    .image;
    assert!(red_at(&plain, 15, 5), "the bound width of the template");
    assert!(empty_at(&plain, 25, 5));
    let narrow = TestWidget::clone_of("copy", "tmpl").with(
        "modifiers",
        clone_modifiers(&[TestModifier::Number("SizeX", 5.0)]),
    );
    let overridden = compose(&template().widget(narrow), &[])?
        .rasterize(&stage())?
        .image;
    assert!(red_at(&overridden, 2, 5));
    assert!(
        empty_at(&overridden, 8, 5),
        "the clone's 5 beats the bound 20"
    );
    Ok(())
}
