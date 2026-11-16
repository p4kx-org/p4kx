// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Report and pixel evidence for the source-based layout approximations.

use std::cell::Cell;

use p4kx_dataforge::DataForge;
use p4kx_screens::{Limits, Report, ScreenError, Still, StillFormat, StillRequest};
use p4kx_testkit::{TestCanvas, TestColour, TestDim, TestValue, TestWidget};

use crate::support::{
    BLUE, GREEN, RED, SCREEN, TestResult, blue_at, compose, compose_with, database, empty_at,
    green_at, guid_text, red_at, stage,
};

fn canvas() -> TestCanvas {
    TestCanvas::new(SCREEN, "approximations", 100.0, 50.0)
}

fn child(name: &str, parent: &str, width: f32, height: f32, colour: TestColour) -> TestWidget {
    TestWidget::display(name)
        .parent(parent)
        .size(TestDim::Fixed(width), TestDim::Fixed(height))
        .background(colour)
}

fn has_note(report: &Report, container: &str, field: &str) -> bool {
    report.unsupported.iter().any(|note| {
        note.reason.contains(field)
            && note.examples.iter().any(|site| {
                site.strip_prefix(container)
                    .is_some_and(|location| location.starts_with(" (Record "))
            })
    })
}

fn stretched() -> TestCanvas {
    canvas()
        .widget(TestWidget::display("stretch").flex("Row", "Stretch", "Start", 0.0))
        .widget(child("red", "stretch", 10.0, 10.0, RED))
        .widget(child("green", "stretch", 10.0, 10.0, GREEN))
}

#[test]
fn fixed_items_grow_only_where_stretch_actually_changes_their_extent() -> TestResult {
    let composed = compose(&stretched())?;
    let image = composed.rasterize(&stage())?.image;
    assert!(red_at(&image, 45, 5), "fixed first item takes half the row");
    assert!(green_at(&image, 55, 5), "fixed second item follows it");
    let field = "layoutPolicy.axisJustification=Stretch";
    assert!(
        has_note(composed.report(), "stretch", field),
        "fixed-item stretch is reported"
    );

    let bounded = compose(
        &canvas()
            .widget(TestWidget::display("capped").flex("Row", "Stretch", "Start", 0.0))
            .widget(child("red", "capped", 10.0, 10.0, RED).max_width(TestDim::Fixed(10.0))),
    )?;
    let image = bounded.rasterize(&stage())?.image;
    assert!(red_at(&image, 8, 5), "item keeps its authored width");
    assert!(
        empty_at(&image, 20, 5),
        "the maximum caps the inferred stretch"
    );
    assert!(
        !has_note(bounded.report(), "capped", field),
        "unchanged size needs no approximation"
    );
    Ok(())
}

#[test]
fn a_single_centred_line_aligns_its_shorter_child_inside_the_line() -> TestResult {
    let composed = compose(
        &canvas()
            .widget(
                TestWidget::display("single")
                    .flex("Row", "Start", "End", 0.0)
                    .lines("Center"),
            )
            .widget(child("tall", "single", 10.0, 20.0, GREEN))
            .widget(child("short", "single", 10.0, 8.0, RED)),
    )?;
    let image = composed.rasterize(&stage())?.image;
    assert!(
        red_at(&image, 15, 30),
        "short child ends at the centred line's edge"
    );
    assert!(
        empty_at(&image, 15, 46),
        "full-width cross allocation would align it here"
    );
    assert!(
        has_note(
            composed.report(),
            "single",
            "layoutPolicy.crossAxisJustification"
        ),
        "the report identifies the single-line cross policy decision"
    );
    Ok(())
}

#[test]
fn wrap_reverse_starts_three_lines_at_the_cross_end() -> TestResult {
    let composed = compose(
        &canvas()
            .widget(
                TestWidget::display("wrapped")
                    .flex("Row", "Start", "Start", 0.0)
                    .wrap("WrapReverse"),
            )
            .widget(child("first", "wrapped", 60.0, 8.0, RED))
            .widget(child("second", "wrapped", 60.0, 8.0, GREEN))
            .widget(child("third", "wrapped", 60.0, 8.0, BLUE)),
    )?;
    let image = composed.rasterize(&stage())?.image;
    assert!(red_at(&image, 5, 46), "the first line is at the far end");
    assert!(green_at(&image, 5, 38), "the second line sits before it");
    assert!(blue_at(&image, 5, 30), "the third line is first");
    assert!(
        empty_at(&image, 5, 4),
        "reverse ordering alone would leave a line here"
    );
    assert!(
        has_note(
            composed.report(),
            "wrapped",
            "layoutPolicy.wrap=WrapReverse"
        ),
        "the report identifies the inferred origin of the reversed lines"
    );
    Ok(())
}

#[test]
fn a_stack_keeps_the_shorter_auto_cross_child_at_its_measured_width() -> TestResult {
    let composed = compose(
        &canvas()
            .widget(
                TestWidget::display("stack")
                    .size(TestDim::Auto, TestDim::Fixed(40.0))
                    .stack("Vertical", 0.0),
            )
            .widget(
                TestWidget::display("auto")
                    .parent("stack")
                    .size(TestDim::Auto, TestDim::Fixed(10.0))
                    .background(RED),
            )
            .widget(child("content", "auto", 12.0, 5.0, BLUE))
            .widget(child("width", "stack", 40.0, 10.0, GREEN)),
    )?;
    let image = composed.rasterize(&stage())?.image;
    assert!(
        red_at(&image, 8, 8),
        "the automatic item includes its content's width"
    );
    assert!(
        empty_at(&image, 30, 8),
        "the automatic item does not fill the stack"
    );
    assert!(
        green_at(&image, 30, 14),
        "the wider sibling defines the stack's cross extent"
    );
    assert!(
        has_note(composed.report(), "stack", "Stack layoutPolicy.direction"),
        "the report identifies the inferred automatic cross size"
    );
    Ok(())
}

#[test]
fn layout_reports_are_deterministic_and_respect_the_widget_limit() -> TestResult {
    let fixture = stretched();
    assert_eq!(
        compose(&fixture)?.report(),
        compose(&fixture)?.report(),
        "same authored layout yields the same report"
    );
    let limited = compose_with(
        &fixture,
        Vec::new(),
        Limits {
            widgets: 2,
            ..Limits::DEFAULT
        },
    )?;
    assert!(
        limited.report().truncated,
        "the child group exceeds the allowed visits"
    );
    assert!(
        !has_note(
            limited.report(),
            "stretch",
            "layoutPolicy.axisJustification"
        ),
        "the unplaced child group reports no inferred placement"
    );
    Ok(())
}

#[test]
fn the_group_lists_all_original_layout_sites_even_when_names_repeat() -> TestResult {
    let fixture = (0..5).fold(canvas(), |fixture, index| {
        let label = format!("container-{index}");
        fixture
            .widget(
                TestWidget::display(&label)
                    .flex("Row", "Stretch", "Start", 0.0)
                    .with("name", TestValue::text("shared name")),
            )
            .widget(child(&format!("item-{index}"), &label, 10.0, 10.0, RED))
    });
    let composed = compose(&fixture)?;
    let grouped = composed
        .report()
        .unsupported
        .iter()
        .find(|row| {
            row.reason
                .contains("layoutPolicy.axisJustification=Stretch")
        })
        .ok_or("no grouped Stretch diagnostic")?;
    let expected: Vec<String> = (0..5)
        .map(|index| format!("shared name (Record 0, scene[{}])", 2 * index))
        .collect();
    assert_eq!(grouped.count, 5, "each container visit is counted");
    assert_eq!(
        grouped.examples, expected,
        "every original slot is retained in visit order"
    );
    Ok(())
}

#[test]
fn cancelling_after_placement_returns_no_still_with_a_partial_layout_report() -> TestResult {
    let bytes = database([stretched().record()]);
    let forge = DataForge::open(&bytes)?;
    let request = StillRequest {
        screen: &guid_text(SCREEN),
        brand: None,
        variables: &[],
        raster: stage(),
        format: StillFormat::Png,
    };
    let started = Cell::new(false);
    let result = Still::render_while(&forge, None, None, &request, &|| started.replace(true));
    assert!(
        matches!(result, Err(ScreenError::Stopped)),
        "the cancellation checkpoint after composition stops publication"
    );
    Ok(())
}
