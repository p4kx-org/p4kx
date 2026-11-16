// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Game variables a caller sets before the bindings are evaluated.

use p4kx_dataforge::DataForge;
use p4kx_screens::{
    Composed, ContentOmissionCode, MAX_VARIABLES, ScreenComposer, ScreenError, Variable,
    VariableInForce,
};
use p4kx_testkit::{TestCanvas, TestDim, TestInstance, TestOp, TestValue, TestWidget};

use crate::support::{
    RED, SCREEN, TestResult, database, empty_at, guid_text, near, pixel, red_at, stage,
};

const INNER: u128 = SCREEN + 0x100;

fn canvas() -> TestCanvas {
    TestCanvas::new(SCREEN, "variables", 10.0, 10.0)
}

const fn square(widget: TestWidget) -> TestWidget {
    widget.size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
}

fn variables(settings: &[&str]) -> Result<Vec<Variable>, Box<dyn std::error::Error>> {
    settings
        .iter()
        .map(|setting| Ok(setting.parse::<Variable>()?))
        .collect()
}

fn compose_with(
    records: Vec<p4kx_testkit::TestRecord>,
    settings: &[&str],
) -> Result<Composed, Box<dyn std::error::Error>> {
    let bytes = database(records);
    let forge = DataForge::open(&bytes)?;
    Ok(ScreenComposer::new(&forge)
        .with_variables(&variables(settings)?)
        .compose(&guid_text(SCREEN), None)?)
}

fn powered_screen() -> TestCanvas {
    canvas()
        .operation("power", TestOp::NamedNumVar("PowerState"))
        .operation("on", TestOp::NumIs("power", "Greater", 0.0))
        .widget(square(TestWidget::display("mfd").background(RED)))
        .operation("show", TestOp::BoolField("mfd", "Instantiated", "on"))
}

fn composed(settings: &[&str]) -> Result<Composed, Box<dyn std::error::Error>> {
    compose_with(vec![powered_screen().record()], settings)
}

#[test]
fn a_variable_that_is_set_turns_the_screen_on_and_the_report_lists_it() -> TestResult {
    let off = composed(&[])?;
    assert!(empty_at(&off.rasterize(&stage())?.image, 5, 5));
    assert_eq!(off.report().variables.first(), None);
    let on = composed(&["powerstate=1"])?;
    assert!(red_at(&on.rasterize(&stage())?.image, 5, 5));
    assert_eq!(
        on.report().variables,
        [VariableInForce {
            name: "powerstate".to_owned(),
            value: "1".to_owned(),
            used: true,
        }]
    );
    assert_eq!(on.report().unknown_variables().count(), 0);
    assert!(
        on.report()
            .unsupported
            .iter()
            .all(|entry| !entry.reason.starts_with("not drawn at rest")),
        "a screen that is drawn is not listed as gone"
    );
    Ok(())
}

#[test]
fn a_name_is_the_last_segment_of_the_path_a_binding_reads() -> TestResult {
    let screen = canvas()
        .operation("power", TestOp::NamedNumVar("/seatdashboard/powerstate"))
        .operation("on", TestOp::NumIs("power", "Greater", 0.0))
        .widget(square(TestWidget::display("mfd").background(RED)))
        .operation("show", TestOp::BoolField("mfd", "Instantiated", "on"));
    for (setting, drawn) in [
        ("powerstate=1", true),
        ("/seatdashboard/powerstate=1", true),
        ("/other/powerstate=1", false),
    ] {
        let composed = compose_with(vec![screen.record()], &[setting])?;
        let image = composed.rasterize(&stage())?.image;
        assert_eq!(red_at(&image, 5, 5), drawn, "{setting}");
        assert_eq!(composed.report().unknown_variables().count() == 0, drawn);
    }
    Ok(())
}

#[test]
fn a_variable_that_no_canvas_reads_is_reported_and_changes_nothing() -> TestResult {
    let composed = composed(&["powerstat=1", "powerstate=0"])?;
    assert!(empty_at(&composed.rasterize(&stage())?.image, 5, 5));
    assert_eq!(
        composed.report().unknown_variables().collect::<Vec<_>>(),
        ["powerstat"]
    );
    Ok(())
}

#[test]
fn the_last_setting_of_a_name_wins_whatever_its_case() -> TestResult {
    let composed = composed(&["powerstate=0", "POWERSTATE=1"])?;
    assert!(red_at(&composed.rasterize(&stage())?.image, 5, 5));
    assert_eq!(composed.report().variables.len(), 1);
    assert_eq!(composed.report().variables[0].value, "1");
    Ok(())
}

#[test]
fn a_value_of_the_wrong_kind_is_listed_and_the_widget_keeps_its_own() -> TestResult {
    let composed = composed(&["powerstate=on"])?;
    assert!(
        red_at(&composed.rasterize(&stage())?.image, 5, 5),
        "the binding is unknown, so the widget stays"
    );
    assert!(
        composed
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.element == "Binding" && entry.reason.contains("not a number")),
        "{:?}",
        composed.report().unsupported
    );
    Ok(())
}

#[test]
fn a_numeric_field_accepts_zero_and_diagnoses_an_invalid_setting() -> TestResult {
    let canvas = canvas()
        .operation("level", TestOp::NamedNumVar("/seatdashboard/level"))
        .widget(square(TestWidget::display("indicator").background(RED)))
        .operation("alpha", TestOp::NumField("indicator", "Alpha", "level"));
    let zero = compose_with(vec![canvas.record()], &["level=0"])?;
    assert!(empty_at(&zero.rasterize(&stage())?.image, 5, 5));
    assert_eq!(
        zero.report().content_omissions,
        [],
        "explicit input has no omission"
    );
    assert_eq!(zero.report().variables[0].value, "0");

    let invalid = compose_with(vec![canvas.record()], &["level=bad"])?;
    let issue = invalid
        .report()
        .content_omissions
        .first()
        .ok_or("the invalid number is not listed")?;
    assert_eq!(issue.widget, "indicator");
    assert_eq!(issue.field, "Alpha");
    assert_eq!(issue.source.as_deref(), Some("/seatdashboard/level"));
    assert_eq!(issue.code, ContentOmissionCode::UnresolvedInput);
    assert!(red_at(&invalid.rasterize(&stage())?.image, 5, 5));
    Ok(())
}

fn scalar_fields(alpha: TestOp, visibility: TestOp) -> TestCanvas {
    let bar = |name, x| {
        TestWidget::display(name)
            .size(TestDim::Fixed(3.0), TestDim::Fixed(10.0))
            .offset(x, 0.0)
            .background(RED)
    };
    canvas()
        .operation("alpha_input", alpha)
        .operation("visibility_input", visibility)
        .widget(bar("fade", 0.0).alpha(0.5))
        .widget(bar("instantiation", 3.0))
        .widget(bar("activity", 6.0))
        .operation("fade", TestOp::NumField("fade", "Alpha", "alpha_input"))
        .operation(
            "instantiate",
            TestOp::BoolField("instantiation", "Instantiated", "visibility_input"),
        )
        .operation(
            "activate",
            TestOp::BoolField("activity", "IsActive", "visibility_input"),
        )
}

#[test]
fn a_mismatched_scalar_binding_reports_its_widget_and_preserves_its_saved_state() -> TestResult {
    let screen = scalar_fields(
        TestOp::NamedLocalVar("/seatdashboard/label"),
        TestOp::NamedNumVar("/seatdashboard/level"),
    );
    let composed = compose_with(vec![screen.record()], &["label=0", "level=0"])?;
    let image = composed.rasterize(&stage())?.image;
    assert!(
        near(pixel(&image, 1, 5), [255, 0, 0, 128], 3),
        "saved alpha"
    );
    assert!(red_at(&image, 4, 5), "saved instantiation");
    assert!(red_at(&image, 7, 5), "saved activity");

    let omissions = &composed.report().content_omissions;
    let actual = omissions
        .iter()
        .map(|row| {
            (
                row.widget.as_str(),
                row.widget_input.slot,
                row.field.as_str(),
                row.source.as_deref(),
                row.code,
            )
        })
        .collect::<Vec<_>>();
    let unresolved = ContentOmissionCode::UnresolvedInput;
    assert_eq!(
        actual,
        [
            ("fade", 0, "Alpha", Some("/seatdashboard/label"), unresolved),
            (
                "instantiation",
                1,
                "Instantiated",
                Some("/seatdashboard/level"),
                unresolved,
            ),
            (
                "activity",
                2,
                "IsActive",
                Some("/seatdashboard/level"),
                unresolved
            ),
        ],
        "each mismatch belongs to its authored slot, field and variable"
    );
    Ok(())
}

#[test]
fn scalar_bindings_accept_explicit_zero_and_false() -> TestResult {
    let screen = scalar_fields(
        TestOp::NamedNumVar("/seatdashboard/level"),
        TestOp::NamedBoolVar("/seatdashboard/visible"),
    );
    let composed = compose_with(vec![screen.record()], &["level=0", "visible=false"])?;
    let image = composed.rasterize(&stage())?.image;
    assert!(empty_at(&image, 1, 5), "zero alpha");
    assert!(empty_at(&image, 4, 5), "false instantiation");
    assert!(empty_at(&image, 7, 5), "false activity");
    assert_eq!(
        composed.report().content_omissions,
        [],
        "valid scalar inputs produce no omissions"
    );
    Ok(())
}

#[test]
fn a_binding_after_a_null_widget_reports_its_actual_physical_target() -> TestResult {
    let widget = |name| {
        TestValue::Struct(
            TestInstance::new("BuildingBlocks_DisplayWidget")
                .named(name)
                .value("name", TestValue::text(name)),
        )
    };
    let level = TestInstance::new("BuildingBlocks_BindingsNumberVariable")
        .named("level")
        .value("binding", TestValue::text("level"));
    let alpha = TestInstance::new("BuildingBlocks_BindingsNumberField")
        .value("widget", TestValue::pointer("target"))
        .value("field", TestValue::text("Alpha"))
        .value("input", TestValue::pointer("level"));
    let record = canvas()
        .record()
        .value(
            "scene",
            TestValue::Array(vec![TestValue::Null, widget("target"), widget("following")]),
        )
        .value(
            "operations",
            TestValue::Array(vec![TestValue::Struct(level), TestValue::Struct(alpha)]),
        );
    let composed = compose_with(vec![record], &["level=bad"])?;
    let issue = composed
        .report()
        .content_omissions
        .first()
        .ok_or("the invalid setting is not listed")?;
    assert_eq!(issue.widget, "target");
    assert_eq!(issue.widget_input.slot, 1);
    assert_eq!(issue.field, "Alpha");
    assert_eq!(issue.source.as_deref(), Some("level"));
    Ok(())
}

#[test]
fn a_variable_of_a_canvas_that_is_not_entered_is_still_known() -> TestResult {
    let inner = TestCanvas::new(INNER, "inner", 10.0, 10.0)
        .operation("deep", TestOp::NamedNumVar("deep"))
        .operation("deeper", TestOp::NumIs("deep", "Greater", 0.0))
        .widget(square(TestWidget::display("content").background(RED)))
        .operation("bind", TestOp::BoolField("content", "IsActive", "deeper"));
    let outer = canvas()
        .operation("flag", TestOp::BoolVar)
        .widget(
            square(TestWidget::new("BuildingBlocks_WidgetCanvas", "holder"))
                .with("canvas", TestValue::Reference(INNER)),
        )
        .operation("hide", TestOp::BoolField("holder", "Instantiated", "flag"));
    let composed = compose_with(vec![outer.record(), inner.record()], &["deep=1", "other=1"])?;
    assert_eq!(
        composed.report().unknown_variables().collect::<Vec<_>>(),
        ["other"]
    );
    Ok(())
}

#[test]
fn more_variables_than_the_limit_are_refused() -> TestResult {
    let settings: Vec<String> = (0..=MAX_VARIABLES).map(|at| format!("v{at}=1")).collect();
    let settings: Vec<&str> = settings.iter().map(String::as_str).collect();
    match composed(&settings) {
        Err(error) => match error.downcast_ref::<ScreenError>() {
            Some(ScreenError::TooManyVariables { count, max }) => {
                assert_eq!((*count, *max), (MAX_VARIABLES + 1, MAX_VARIABLES));
                Ok(())
            }
            other => Err(format!("another error: {other:?}").into()),
        },
        Ok(_) => Err("the settings were taken".into()),
    }
}
