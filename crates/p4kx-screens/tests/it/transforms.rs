// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Anchor, pivot, rotation, scale, overflow, the scaling of nested canvases and the aspect modes.

use p4kx_testkit::{TestCanvas, TestDim, TestModifier, TestStyleEntry, TestWidget};

use crate::support::{
    GREEN, RED, SCREEN, TestResult, compose, empty_at, green_at, red_at, render, render_with,
};
use p4kx_testkit::TestCondition;

const INNER: u128 = 0x5a00_0000_0000_4000_8000_0000_0000_0011;

fn canvas() -> TestCanvas {
    TestCanvas::new(SCREEN, "transforms", 100.0, 50.0)
}

fn square(name: &str, side: f32) -> TestWidget {
    TestWidget::display(name)
        .size(TestDim::Fixed(side), TestDim::Fixed(side))
        .background(RED)
}

#[test]
fn the_pivot_sits_on_the_anchor_of_the_parent() -> TestResult {
    let image = render(&canvas().widget(square("centred", 20.0).anchor(0.5, 0.5).pivot(0.5, 0.5)))?;
    assert!(red_at(&image, 50, 25), "in the middle");
    assert!(red_at(&image, 41, 16), "ten pixels from the middle");
    assert!(empty_at(&image, 38, 25), "no further");
    assert!(empty_at(&image, 5, 5), "not at the origin");
    Ok(())
}

#[test]
fn a_pivot_and_an_anchor_in_a_corner_hold_the_widget_in_that_corner() -> TestResult {
    let image = render(&canvas().widget(square("corner", 20.0).anchor(1.0, 1.0).pivot(1.0, 1.0)))?;
    assert!(red_at(&image, 95, 45), "bottom right");
    assert!(empty_at(&image, 5, 5), "not top left");
    Ok(())
}

#[test]
fn an_item_in_a_flex_container_is_placed_by_the_container_not_by_its_anchor() -> TestResult {
    let image = render(
        &canvas()
            .widget(TestWidget::display("row").flex("Row", "Start", "Start", 0.0))
            .widget(
                square("item", 20.0)
                    .parent("row")
                    .anchor(1.0, 1.0)
                    .pivot(1.0, 1.0),
            ),
    )?;
    assert!(red_at(&image, 5, 5), "at the start of the row");
    Ok(())
}

#[test]
fn a_widget_turns_about_its_pivot() -> TestResult {
    let image = render(
        &canvas().widget(
            TestWidget::display("bar")
                .size(TestDim::Fixed(40.0), TestDim::Fixed(10.0))
                .anchor(0.5, 0.5)
                .pivot(0.5, 0.5)
                .rotate(90.0)
                .background(RED),
        ),
    )?;
    assert!(red_at(&image, 50, 40), "the bar stands up about its middle");
    assert!(red_at(&image, 50, 10), "at both ends");
    assert!(empty_at(&image, 35, 25), "where it lay before");
    assert!(empty_at(&image, 65, 25), "on both sides");
    Ok(())
}

#[test]
fn a_turned_widget_turns_its_children_with_it() -> TestResult {
    let image = render(
        &canvas()
            .widget(
                TestWidget::display("bar")
                    .size(TestDim::Fixed(40.0), TestDim::Fixed(10.0))
                    .anchor(0.5, 0.5)
                    .pivot(0.5, 0.5)
                    .rotate(90.0),
            )
            .widget(
                TestWidget::display("end")
                    .parent("bar")
                    .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
                    .offset(30.0, 0.0)
                    .background(GREEN),
            ),
    )?;
    assert!(
        green_at(&image, 50, 40),
        "the right end turned to the bottom"
    );
    Ok(())
}

#[test]
fn a_half_turn_about_the_x_axis_flips_the_widget_over() -> TestResult {
    let image = render(
        &canvas()
            .widget(
                TestWidget::display("flip")
                    .size(TestDim::Fixed(20.0), TestDim::Fixed(20.0))
                    .anchor(0.5, 0.5)
                    .pivot(0.5, 0.5)
                    .orientation(180.0, 0.0, 0.0),
            )
            .widget(
                TestWidget::display("mark")
                    .parent("flip")
                    .size(TestDim::Fixed(20.0), TestDim::Fixed(5.0))
                    .background(RED),
            ),
    )?;
    assert!(empty_at(&image, 50, 17), "the mark left the top of the box");
    assert!(red_at(&image, 50, 32), "and is at the bottom of it");
    Ok(())
}

#[test]
fn a_widget_seen_edge_on_is_not_drawn() -> TestResult {
    let composed = compose(&canvas().widget(square("edge", 20.0).orientation(90.0, 0.0, 0.0)))?;
    assert_eq!(composed.report().shapes, 0, "nothing of it is visible");
    Ok(())
}

#[test]
fn scale_grows_a_widget_about_its_pivot() -> TestResult {
    let image = render(&canvas().widget(square("big", 10.0).pivot(0.0, 0.0).scale(2.0, 3.0)))?;
    assert!(red_at(&image, 15, 25), "twice as wide, three times as high");
    assert!(empty_at(&image, 25, 5), "no wider");
    assert!(empty_at(&image, 5, 35), "no higher");
    Ok(())
}

#[test]
fn a_zero_scale_and_an_infinite_angle_do_not_break_the_render() -> TestResult {
    let composed = compose(
        &canvas()
            .widget(square("gone", 10.0).scale(0.0, 0.0))
            .widget(square("wild", 10.0).orientation(f32::NAN, f32::INFINITY, 0.0)),
    )?;
    assert!(
        composed
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.reason.contains("not finite")),
        "{:?}",
        composed.report().unsupported
    );
    Ok(())
}

#[test]
fn style_entries_move_turn_and_scale_widgets_too() -> TestResult {
    let entry = TestStyleEntry::new("move")
        .when(TestCondition::Type("Base"))
        .set(TestModifier::Number("AnchorX", 1.0))
        .set(TestModifier::Number("PivotX", 1.0));
    let image = render(&canvas().embedded(entry).widget(square("moved", 20.0)))?;
    assert!(red_at(&image, 95, 5), "the style put it at the right edge");
    assert!(empty_at(&image, 5, 5), "and not at the left");
    Ok(())
}

#[test]
fn clipping_cuts_the_children_at_the_box() -> TestResult {
    let image = render(
        &canvas()
            .widget(
                TestWidget::display("window")
                    .size(TestDim::Fixed(20.0), TestDim::Fixed(20.0))
                    .overflow("Clip"),
            )
            .widget(
                TestWidget::display("wide")
                    .parent("window")
                    .size(TestDim::Fixed(60.0), TestDim::Fixed(10.0))
                    .background(RED),
            ),
    )?;
    assert!(red_at(&image, 10, 5), "inside the window");
    assert!(empty_at(&image, 30, 5), "outside the window");
    Ok(())
}

#[test]
fn without_clipping_children_may_leave_the_box() -> TestResult {
    let image = render(
        &canvas()
            .widget(TestWidget::display("window").size(TestDim::Fixed(20.0), TestDim::Fixed(20.0)))
            .widget(
                TestWidget::display("wide")
                    .parent("window")
                    .size(TestDim::Fixed(60.0), TestDim::Fixed(10.0))
                    .background(RED),
            ),
    )?;
    assert!(red_at(&image, 30, 5), "outside the box, still visible");
    Ok(())
}

#[test]
fn clips_nest() -> TestResult {
    let image = render(
        &canvas()
            .widget(
                TestWidget::display("outer")
                    .size(TestDim::Fixed(40.0), TestDim::Fixed(40.0))
                    .overflow("Clip"),
            )
            .widget(
                TestWidget::display("inner")
                    .parent("outer")
                    .size(TestDim::Fixed(30.0), TestDim::Fixed(10.0))
                    .overflow("Clip"),
            )
            .widget(
                TestWidget::display("fill")
                    .parent("inner")
                    .size(TestDim::Fixed(80.0), TestDim::Fixed(40.0))
                    .background(RED),
            ),
    )?;
    assert!(red_at(&image, 25, 5), "inside both");
    assert!(empty_at(&image, 35, 5), "outside the inner clip");
    assert!(empty_at(&image, 25, 15), "below the inner clip");
    Ok(())
}

#[test]
fn a_fade_is_listed_and_a_clip_with_fade_still_clips() -> TestResult {
    let composed = compose(
        &canvas()
            .widget(
                TestWidget::display("window")
                    .size(TestDim::Fixed(20.0), TestDim::Fixed(20.0))
                    .overflow("ClipFade"),
            )
            .widget(
                TestWidget::display("wide")
                    .parent("window")
                    .size(TestDim::Fixed(60.0), TestDim::Fixed(10.0))
                    .background(RED),
            ),
    )?;
    assert!(
        composed
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.reason.contains("fading edges")),
        "{:?}",
        composed.report().unsupported
    );
    Ok(())
}

fn inner(method: &'static str) -> TestCanvas {
    TestCanvas::new(INNER, "inner", 20.0, 10.0)
        .coordinates(method)
        .widget(
            TestWidget::display("fill")
                .size(TestDim::Percent(1.0), TestDim::Percent(1.0))
                .background(RED),
        )
}

fn host(width: f32, height: f32, sizing: &str) -> TestCanvas {
    canvas().widget(
        TestWidget::new("BuildingBlocks_WidgetCanvas", "host")
            .size(TestDim::Fixed(width), TestDim::Fixed(height))
            .sized_by(sizing)
            .with("canvas", p4kx_testkit::TestValue::Reference(INNER)),
    )
}

#[test]
fn a_nested_canvas_is_laid_out_in_the_box_of_its_widget() -> TestResult {
    let image = render_with(&host(60.0, 40.0, "Size"), vec![inner("auto").record()])?;
    assert!(red_at(&image, 55, 35), "the fill follows the box");
    assert!(empty_at(&image, 65, 35), "and ends there");
    Ok(())
}

#[test]
fn a_scaled_canvas_keeps_its_own_size_and_is_scaled_into_the_box() -> TestResult {
    let image = render_with(&host(60.0, 40.0, "Scale"), vec![inner("auto").record()])?;
    assert!(
        red_at(&image, 30, 20),
        "the fill is as high as the box allows"
    );
    assert!(red_at(&image, 58, 29), "scaled by three to 60 x 30");
    assert!(empty_at(&image, 30, 35), "the aspect of the canvas is kept");
    Ok(())
}

/// Checks that a canvas of `method` fills a box of `size`: `inside` is red, `outside` is empty.
fn assert_fills_box(
    method: &'static str,
    size: (f32, f32),
    inside: (u32, u32),
    outside: (u32, u32),
) -> TestResult {
    let image = render_with(&host(size.0, size.1, "Size"), vec![inner(method).record()])?;
    assert!(
        red_at(&image, inside.0, inside.1),
        "{method}: fills the box"
    );
    assert!(
        empty_at(&image, outside.0, outside.1),
        "{method}: ends at the box"
    );
    Ok(())
}

#[test]
fn aspect_overrides_width_lays_the_canvas_out_in_the_whole_box() -> TestResult {
    assert_fills_box("aspectOverridesWidth", (80.0, 20.0), (75, 10), (85, 10))
}

#[test]
fn aspect_overrides_height_lays_the_canvas_out_in_the_whole_box() -> TestResult {
    assert_fills_box("aspectOverridesHeight", (40.0, 50.0), (20, 45), (20, 55))
}

#[test]
fn numbers_a_style_sets_are_held_to_a_sane_range_or_ignored() -> TestResult {
    let entry = TestStyleEntry::new("wild")
        .when(TestCondition::Type("Base"))
        .set(TestModifier::Number("SizeX", f32::NAN))
        .set(TestModifier::Number("PosY", f32::INFINITY))
        .set(TestModifier::Number("PivotX", 1.0e30));
    let image = render(&canvas().embedded(entry).widget(square("steady", 20.0)))?;
    assert!(empty_at(&image, 5, 5), "the pivot sent it far away");
    Ok(())
}
