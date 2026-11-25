// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Clones, lists and library templates: what they draw at rest and how they are bounded.

use p4kx_screens::{ContentOmissionCode, Limits};
use p4kx_testkit::{TestCanvas, TestDim, TestModifier, TestValue, TestWidget, clone_modifiers};

use crate::support::{
    BLUE, GREEN, RED, SCREEN, TestResult, blue_at, compose, compose_with, empty_at, green_at,
    red_at, render,
};

fn canvas() -> TestCanvas {
    TestCanvas::new(SCREEN, "instances", 100.0, 50.0)
}

fn template(name: &str, width: f32, height: f32) -> TestWidget {
    TestWidget::display(name)
        .size(TestDim::Fixed(width), TestDim::Fixed(height))
        .background(RED)
}

#[test]
fn a_clone_draws_its_template_and_the_children_of_the_template() -> TestResult {
    let image = render(
        &canvas()
            .template(template("tmpl", 40.0, 30.0))
            .widget(
                TestWidget::display("knob")
                    .parent("tmpl")
                    .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
                    .background(GREEN),
            )
            .widget(TestWidget::clone_of("copy", "tmpl")),
    )?;
    assert!(green_at(&image, 5, 5), "the child of the template");
    assert!(red_at(&image, 30, 20), "the template");
    assert!(empty_at(&image, 60, 20), "right of the template");
    Ok(())
}

#[test]
fn a_template_that_nothing_instantiates_is_not_drawn() -> TestResult {
    let image = render(&canvas().template(template("tmpl", 40.0, 30.0)))?;
    assert!(empty_at(&image, 5, 5), "the library is not on the screen");
    Ok(())
}

#[test]
fn two_clones_of_a_template_are_laid_out_by_their_parent() -> TestResult {
    let image = render(
        &canvas()
            .template(template("tmpl", 20.0, 20.0))
            .widget(TestWidget::display("row").flex("Row", "Start", "Start", 10.0))
            .widget(
                TestWidget::clone_of("first", "tmpl")
                    .parent("row")
                    .size(TestDim::Fixed(20.0), TestDim::Fixed(20.0)),
            )
            .widget(
                TestWidget::clone_of("second", "tmpl")
                    .parent("row")
                    .size(TestDim::Fixed(20.0), TestDim::Fixed(20.0)),
            ),
    )?;
    assert!(red_at(&image, 10, 10), "the first clone");
    assert!(empty_at(&image, 25, 10), "the gap");
    assert!(red_at(&image, 40, 10), "the second clone");
    Ok(())
}

#[test]
fn repeated_runtime_images_share_the_template_omission() -> TestResult {
    let picture = |name| {
        TestWidget::new("BuildingBlocks_WidgetRuntimeImage", name)
            .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
            .with("imageSource", TestValue::text("synthetic-render-target"))
    };
    let composed = compose(
        &canvas()
            .template(picture("template"))
            .widget(TestWidget::clone_of("first", "template"))
            .widget(TestWidget::clone_of("second", "template"))
            .widget(picture("standalone")),
    )?;
    let report = composed.report();
    let image_visits = report
        .unsupported
        .iter()
        .find(|entry| entry.element == "WidgetRuntimeImage")
        .ok_or("no grouped runtime image omissions")?;
    assert_eq!(
        image_visits.count, 3,
        "two visits to the template and one standalone"
    );
    let [template, standalone] = report.content_omissions.as_slice() else {
        return Err("expected one detail per authored runtime image".into());
    };
    assert_eq!(template.widget, "template");
    assert_eq!(standalone.widget, "standalone");
    assert_ne!(template.widget_input, standalone.widget_input);
    Ok(())
}

#[test]
fn the_modifiers_of_a_clone_change_its_template() -> TestResult {
    let image = render(&canvas().template(template("tmpl", 40.0, 30.0)).widget(
        TestWidget::clone_of("copy", "tmpl").with(
            "modifiers",
            clone_modifiers(&[
                TestModifier::EnableBackground(true),
                TestModifier::Background(BLUE),
            ]),
        ),
    ))?;
    assert!(blue_at(&image, 20, 15), "the colour the clone sets");
    Ok(())
}

#[test]
fn a_list_repeats_its_template_for_every_row_of_its_static_array() -> TestResult {
    let image = render(
        &canvas()
            .template(template("item", 10.0, 10.0))
            .static_array("rows", 3)
            .widget(TestWidget::list("list", "item", "rows").flex("Row", "Start", "Start", 5.0)),
    )?;
    for x in [5, 20, 35] {
        assert!(red_at(&image, x, 5), "an item at x = {x}");
    }
    assert!(empty_at(&image, 12, 5), "the gap");
    assert!(empty_at(&image, 50, 5), "no fourth item");
    Ok(())
}

#[test]
fn a_list_with_run_time_rows_is_empty_at_rest_and_says_so() -> TestResult {
    let composed = compose(
        &canvas()
            .template(template("item", 10.0, 10.0))
            .widget(TestWidget::list("list", "item", "FlightController/Ticks")),
    )?;
    assert_eq!(composed.report().shapes, 0, "nothing is drawn");
    let note = composed
        .report()
        .unsupported
        .iter()
        .find(|entry| entry.element == "WidgetList")
        .ok_or("the list is not reported")?;
    assert_eq!(
        note.reason,
        "list rows require a run-time array supplied by the game"
    );
    let missing = composed
        .report()
        .content_omissions
        .first()
        .ok_or("no runtime array omission")?;
    assert_eq!(missing.widget, "list");
    assert_eq!(missing.field, "arrayVariable");
    assert_eq!(missing.source.as_deref(), Some("FlightController/Ticks"));
    assert_eq!(missing.code, ContentOmissionCode::RuntimeArray);
    assert!(
        composed
            .report()
            .unsupported
            .iter()
            .all(|entry| !entry.reason.contains("no renderer")),
        "lists and clones have a renderer"
    );
    Ok(())
}

#[test]
fn a_list_stops_at_the_row_limit() -> TestResult {
    let limits = Limits {
        rows: 4,
        ..Limits::DEFAULT
    };
    let composed = compose_with(
        &canvas()
            .template(template("item", 10.0, 10.0))
            .static_array("rows", 50)
            .widget(TestWidget::list("list", "item", "rows").flex("Row", "Start", "Start", 0.0)),
        Vec::new(),
        limits,
    )?;
    assert!(composed.report().truncated, "the rows were cut");
    assert_eq!(composed.report().shapes, 4, "four items");
    Ok(())
}

#[test]
fn a_template_with_a_clone_of_itself_is_repeated_once_and_reported() -> TestResult {
    let composed = compose(
        &canvas()
            .template(template("loop", 40.0, 30.0))
            .widget(TestWidget::clone_of("inside", "loop").parent("loop"))
            .widget(TestWidget::clone_of("outside", "loop")),
    )?;
    assert!(
        composed.report().widgets <= 4,
        "{}",
        composed.report().widgets
    );
    assert!(
        composed
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.reason.contains("clone of itself")),
        "the cycle is reported: {:?}",
        composed.report().unsupported
    );
    Ok(())
}

#[test]
fn templates_that_clone_each_other_in_ten_levels_are_bounded() -> TestResult {
    let levels = 8;
    let mut canvas = canvas();
    for level in 0..levels {
        canvas = canvas.template(template(&format!("t{level}"), 2.0, 2.0));
    }
    for level in 0..levels - 1 {
        for copy in 0..10 {
            canvas = canvas.widget(
                TestWidget::clone_of(format!("c{level}_{copy}"), format!("t{}", level + 1))
                    .parent(format!("t{level}")),
            );
        }
    }
    canvas = canvas.widget(TestWidget::clone_of("start", "t0"));
    let limits = Limits {
        instances: 500,
        ..Limits::DEFAULT
    };
    let composed = compose_with(&canvas, Vec::new(), limits)?;
    assert!(
        composed.report().truncated,
        "ten to the seventh is too many"
    );
    assert!(
        composed.report().widgets <= 4 * 500,
        "{} widgets",
        composed.report().widgets
    );
    Ok(())
}

#[test]
fn a_clone_without_a_target_is_reported_and_draws_nothing() -> TestResult {
    let composed =
        compose(&canvas().widget(TestWidget::new("BuildingBlocks_WidgetClone", "lost")))?;
    assert_eq!(composed.report().shapes, 0);
    assert!(
        composed
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.element == "WidgetClone" && entry.reason.contains("no target")),
        "{:?}",
        composed.report().unsupported
    );
    Ok(())
}

#[test]
fn the_modifiers_of_a_clone_change_the_size_its_container_takes() -> TestResult {
    let image = render(
        &canvas()
            .template(template("tmpl", 40.0, 30.0))
            .widget(
                TestWidget::display("fit")
                    .size(TestDim::Auto, TestDim::Auto)
                    .flex("Row", "Start", "Start", 0.0)
                    .background(GREEN),
            )
            .widget(
                TestWidget::clone_of("copy", "tmpl")
                    .parent("fit")
                    .size(TestDim::Auto, TestDim::Auto)
                    .with(
                        "modifiers",
                        clone_modifiers(&[TestModifier::Number("SizeX", 20.0)]),
                    ),
            ),
    )?;
    assert!(
        red_at(&image, 15, 10),
        "the template, as wide as the modifier says"
    );
    assert!(
        empty_at(&image, 30, 10),
        "the container is not as wide as the template without the modifier"
    );
    Ok(())
}
