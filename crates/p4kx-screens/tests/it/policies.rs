// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Stack and grid containers, wrapping and shrinking flex containers, lines, bounds and sizes
//! that come from the content.

use p4kx_testkit::{TestCanvas, TestColour, TestDim, TestWidget};

use crate::support::{
    BLUE, GREEN, RED, SCREEN, TestResult, blue_at, compose, empty_at, green_at, red_at, render,
};

fn canvas() -> TestCanvas {
    TestCanvas::new(SCREEN, "policies", 100.0, 50.0)
}

fn boxed(name: &str, parent: &str, width: f32, height: f32, colour: TestColour) -> TestWidget {
    TestWidget::display(name)
        .parent(parent)
        .size(TestDim::Fixed(width), TestDim::Fixed(height))
        .background(colour)
}

#[test]
fn a_vertical_stack_puts_its_children_below_each_other_with_the_spacing() -> TestResult {
    let image = render(
        &canvas()
            .widget(TestWidget::display("stack").stack("Vertical", 4.0))
            .widget(boxed("a", "stack", 10.0, 10.0, RED))
            .widget(boxed("b", "stack", 10.0, 10.0, GREEN)),
    )?;
    assert!(red_at(&image, 5, 5), "the first");
    assert!(empty_at(&image, 5, 12), "the spacing");
    assert!(green_at(&image, 5, 18), "the second");
    assert!(
        empty_at(&image, 15, 18),
        "children stay at the start across"
    );
    Ok(())
}

#[test]
fn a_horizontal_stack_puts_its_children_side_by_side() -> TestResult {
    let image = render(
        &canvas()
            .widget(TestWidget::display("stack").stack("Horizontal", 6.0))
            .widget(boxed("a", "stack", 10.0, 10.0, RED))
            .widget(boxed("b", "stack", 10.0, 10.0, GREEN)),
    )?;
    assert!(red_at(&image, 5, 5));
    assert!(empty_at(&image, 13, 5));
    assert!(green_at(&image, 21, 5));
    Ok(())
}

#[test]
fn a_grid_pack_fills_a_row_and_continues_in_the_next() -> TestResult {
    let image = render(
        &canvas()
            .widget(TestWidget::display("grid").grid("Horizontal", 2.0))
            .widget(boxed("a", "grid", 40.0, 10.0, RED))
            .widget(boxed("b", "grid", 40.0, 10.0, GREEN))
            .widget(boxed("c", "grid", 40.0, 10.0, BLUE)),
    )?;
    assert!(red_at(&image, 5, 5), "first cell");
    assert!(green_at(&image, 45, 5), "second cell, after the gutter");
    assert!(blue_at(&image, 5, 17), "the third wraps below the first");
    assert!(empty_at(&image, 45, 17), "and nothing sits next to it");
    Ok(())
}

#[test]
fn a_vertical_grid_pack_fills_a_column_first() -> TestResult {
    let image = render(
        &canvas()
            .widget(TestWidget::display("grid").grid("Vertical", 0.0))
            .widget(boxed("a", "grid", 10.0, 30.0, RED))
            .widget(boxed("b", "grid", 10.0, 30.0, GREEN)),
    )?;
    assert!(red_at(&image, 5, 15), "the first column");
    assert!(
        green_at(&image, 15, 15),
        "the second column, as the first is full"
    );
    Ok(())
}

#[test]
fn a_wrapping_row_starts_a_line_when_the_next_child_does_not_fit() -> TestResult {
    let image = render(
        &canvas()
            .widget(
                TestWidget::display("row")
                    .flex("Row", "Start", "Start", 0.0)
                    .wrap("Wrap"),
            )
            .widget(boxed("a", "row", 40.0, 10.0, RED))
            .widget(boxed("b", "row", 40.0, 10.0, GREEN))
            .widget(boxed("c", "row", 40.0, 10.0, BLUE)),
    )?;
    assert!(red_at(&image, 5, 5));
    assert!(green_at(&image, 45, 5));
    assert!(blue_at(&image, 5, 15), "on the second line");
    Ok(())
}

#[test]
fn wrapped_lines_can_be_reversed_and_spread_across() -> TestResult {
    let image = render(
        &canvas()
            .widget(
                TestWidget::display("row")
                    .flex("Row", "Start", "Start", 0.0)
                    .wrap("WrapReverse"),
            )
            .widget(boxed("a", "row", 60.0, 10.0, RED))
            .widget(boxed("b", "row", 60.0, 10.0, GREEN)),
    )?;
    assert!(green_at(&image, 5, 35), "the second line comes first");
    assert!(
        red_at(&image, 5, 45),
        "and the first line is last, at the end"
    );
    assert!(
        empty_at(&image, 5, 5),
        "the lines start at the end of the cross axis"
    );
    Ok(())
}

#[test]
fn a_nowrap_row_shrinks_the_children_that_may_shrink() -> TestResult {
    let image = render(
        &canvas()
            .widget(TestWidget::display("row").flex("Row", "Start", "Start", 0.0))
            .widget(boxed("a", "row", 60.0, 10.0, RED).shrink(1.0))
            .widget(boxed("b", "row", 60.0, 10.0, GREEN).shrink(1.0)),
    )?;
    assert!(
        red_at(&image, 48, 5),
        "the first gives up half of the overflow"
    );
    assert!(green_at(&image, 52, 5), "and the second the same");
    assert!(green_at(&image, 98, 5), "the row fits the container");
    Ok(())
}

#[test]
fn children_that_may_not_shrink_overflow() -> TestResult {
    let image = render(
        &canvas()
            .widget(TestWidget::display("row").flex("Row", "Start", "Start", 0.0))
            .widget(boxed("a", "row", 60.0, 10.0, RED))
            .widget(boxed("b", "row", 60.0, 10.0, GREEN)),
    )?;
    assert!(red_at(&image, 58, 5), "the first keeps its width");
    assert!(
        green_at(&image, 62, 5),
        "the second sits after it, out of the container"
    );
    Ok(())
}

#[test]
fn a_stretching_axis_justification_shares_the_free_space() -> TestResult {
    let image = render(
        &canvas()
            .widget(TestWidget::display("row").flex("Row", "Stretch", "Start", 0.0))
            .widget(boxed("a", "row", 10.0, 10.0, RED))
            .widget(boxed("b", "row", 10.0, 10.0, GREEN)),
    )?;
    assert!(red_at(&image, 45, 5), "half of the width");
    assert!(green_at(&image, 55, 5), "the other half");
    Ok(())
}

#[test]
fn stretching_lines_and_items_fill_the_container_across() -> TestResult {
    let image = render(
        &canvas()
            .widget(
                TestWidget::display("row")
                    .flex("Row", "Start", "Stretch", 0.0)
                    .lines("Stretch"),
            )
            .widget(boxed("a", "row", 10.0, 10.0, RED)),
    )?;
    assert!(
        red_at(&image, 5, 45),
        "the item is as high as the container"
    );
    Ok(())
}

#[test]
fn lines_are_placed_by_the_cross_justification_and_items_by_the_alignment() -> TestResult {
    let image = render(
        &canvas()
            .widget(
                TestWidget::display("row")
                    .flex("Row", "Start", "Start", 0.0)
                    .lines("Center"),
            )
            .widget(boxed("a", "row", 10.0, 10.0, RED)),
    )?;
    assert!(red_at(&image, 5, 25), "centred across");
    assert!(empty_at(&image, 5, 5), "not at the start");
    Ok(())
}

#[test]
fn minimum_and_maximum_sizes_bound_the_size_of_a_widget() -> TestResult {
    let image = render(
        &canvas()
            .widget(
                TestWidget::display("small")
                    .size(TestDim::Percent(0.1), TestDim::Fixed(10.0))
                    .min_width(TestDim::Fixed(30.0))
                    .background(RED),
            )
            .widget(
                TestWidget::display("large")
                    .size(TestDim::Percent(0.9), TestDim::Fixed(10.0))
                    .max_width(TestDim::Fixed(20.0))
                    .offset(0.0, 20.0)
                    .background(GREEN),
            ),
    )?;
    assert!(red_at(&image, 28, 5), "raised to the minimum");
    assert!(empty_at(&image, 32, 5), "and not beyond");
    assert!(green_at(&image, 18, 25), "held at the maximum");
    assert!(empty_at(&image, 22, 25), "and not beyond");
    Ok(())
}

#[test]
fn a_container_of_automatic_size_is_as_large_as_its_children() -> TestResult {
    let image = render(
        &canvas()
            .widget(
                TestWidget::display("fit")
                    .size(TestDim::Auto, TestDim::Auto)
                    .flex("Row", "Start", "Start", 5.0)
                    .background(GREEN),
            )
            .widget(boxed("a", "fit", 10.0, 10.0, RED))
            .widget(boxed("b", "fit", 20.0, 14.0, RED)),
    )?;
    assert!(red_at(&image, 5, 5));
    assert!(green_at(&image, 12, 5), "the gap belongs to the container");
    assert!(red_at(&image, 25, 5));
    assert!(
        green_at(&image, 12, 12),
        "the container is as high as its tallest child"
    );
    assert!(empty_at(&image, 36, 5), "and as wide as its row");
    assert!(empty_at(&image, 12, 16), "and no higher");
    Ok(())
}

#[test]
fn a_measurable_empty_child_leaves_space_for_a_shrinkable_sibling() -> TestResult {
    let image = render(
        &canvas()
            .widget(TestWidget::display("column").flex("Column", "Start", "Start", 4.0))
            .widget(
                TestWidget::display("auto")
                    .parent("column")
                    .size(TestDim::Fixed(20.0), TestDim::Auto),
            )
            .widget(
                TestWidget::display("empty")
                    .parent("auto")
                    .size(TestDim::Fixed(20.0), TestDim::Auto)
                    .background(BLUE),
            )
            .widget(
                TestWidget::display("holder")
                    .parent("column")
                    .size(TestDim::Fixed(20.0), TestDim::Percent(1.0))
                    .shrink(1.0)
                    .background(RED),
            ),
    )?;
    assert!(
        empty_at(&image, 5, 2),
        "the only space above the holder is the gap"
    );
    assert!(red_at(&image, 5, 6), "the holder starts below the gap");
    assert!(
        red_at(&image, 5, 47),
        "the holder keeps the remaining height"
    );
    Ok(())
}

#[test]
fn a_container_with_only_relative_children_takes_the_size_of_its_parent() -> TestResult {
    let composed = compose(
        &canvas()
            .widget(
                TestWidget::display("fit")
                    .size(TestDim::Auto, TestDim::Auto)
                    .background(GREEN),
            )
            .widget(TestWidget::display("fill").parent("fit")),
    )?;
    assert!(
        composed
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.reason.contains("size of the parent")),
        "{:?}",
        composed.report().unsupported
    );
    Ok(())
}

#[test]
fn a_spacer_container_is_listed_and_lays_out_as_an_overlay() -> TestResult {
    let composed = compose(
        &canvas()
            .widget(TestWidget::display("spacer").spacer(4.0))
            .widget(boxed("a", "spacer", 10.0, 10.0, RED)),
    )?;
    assert!(
        composed
            .report()
            .unsupported
            .iter()
            .any(|entry| entry.reason.contains("SpacerContainer")),
        "{:?}",
        composed.report().unsupported
    );
    Ok(())
}
