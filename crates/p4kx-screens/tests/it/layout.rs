// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Sizing, margins, padding, flex containers and raw units of the layout.

use p4kx_dataforge::DataForge;
use p4kx_gfx::RgbaImage;
use p4kx_screens::ScreenComposer;
use p4kx_testkit::{TestCanvas, TestColour, TestDim, TestWidget};

use crate::support::{TestResult, database, guid_text, near, pixel, stage};

const CANVAS: u128 = 0x5a00_0000_0000_4000_8000_0000_0000_0001;
const RED: TestColour = TestColour::Solid([255, 0, 0, 255]);
const GREEN: TestColour = TestColour::Solid([0, 255, 0, 255]);
const BLUE: TestColour = TestColour::Solid([0, 0, 255, 255]);
const NOTHING: [u8; 4] = [0, 0, 0, 0];

#[expect(
    clippy::needless_pass_by_value,
    reason = "every test hands over the canvas it builds"
)]
fn render(canvas: TestCanvas) -> Result<RgbaImage, Box<dyn std::error::Error>> {
    let bytes = database([canvas.record()]);
    let forge = DataForge::open(&bytes)?;
    let composed = ScreenComposer::new(&forge).compose(&guid_text(CANVAS), None)?;
    Ok(composed.rasterize(&stage())?.image)
}

fn boxes(count: u8, direction: &str, justification: &str, gap: f32) -> TestCanvas {
    let colours = [RED, GREEN, BLUE];
    let root = TestWidget::display("root").flex(direction, justification, "Start", gap);
    (0..count)
        .filter_map(|at| {
            colours
                .get(usize::from(at))
                .copied()
                .map(|colour| (at, colour))
        })
        .fold(
            TestCanvas::new(CANVAS, "layout", 100.0, 50.0).widget(root),
            |canvas, (at, colour)| {
                canvas.widget(
                    TestWidget::display(format!("box{at}"))
                        .parent("root")
                        .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
                        .background(colour),
                )
            },
        )
}

#[test]
fn percent_and_fixed_sizes_fill_the_parent() -> TestResult {
    let image = render(
        TestCanvas::new(CANVAS, "layout", 100.0, 50.0).widget(
            TestWidget::display("half")
                .size(TestDim::Percent(0.5), TestDim::Fixed(20.0))
                .background(RED),
        ),
    )?;
    assert!(
        near(pixel(&image, 25, 10), [255, 0, 0, 255], 0),
        "inside the box"
    );
    assert_eq!(pixel(&image, 75, 10), NOTHING, "right of the box");
    assert_eq!(pixel(&image, 25, 30), NOTHING, "below the box");
    Ok(())
}

#[test]
fn space_between_puts_the_first_and_last_item_at_the_ends() -> TestResult {
    let image = render(boxes(3, "Row", "SpaceBetween", 0.0))?;
    for x in [5, 50, 95] {
        assert_ne!(pixel(&image, x, 5), NOTHING, "an item at x = {x}");
    }
    assert_eq!(
        pixel(&image, 25, 5),
        NOTHING,
        "free space between the items"
    );
    Ok(())
}

#[test]
fn a_reversed_row_starts_with_the_last_item() -> TestResult {
    let image = render(boxes(2, "RowReverse", "Start", 0.0))?;
    assert!(
        near(pixel(&image, 5, 5), [0, 255, 0, 255], 0),
        "the second box first"
    );
    assert!(
        near(pixel(&image, 15, 5), [255, 0, 0, 255], 0),
        "then the first"
    );
    Ok(())
}

#[test]
fn a_column_with_a_gap_stacks_downwards() -> TestResult {
    let image = render(boxes(2, "Column", "Start", 5.0))?;
    assert!(
        near(pixel(&image, 5, 5), [255, 0, 0, 255], 0),
        "the first box at the top"
    );
    assert_eq!(pixel(&image, 5, 12), NOTHING, "the gap");
    assert!(
        near(pixel(&image, 5, 20), [0, 255, 0, 255], 0),
        "the second box after the gap"
    );
    Ok(())
}

#[test]
fn items_share_the_free_space_by_their_grow() -> TestResult {
    let canvas = TestCanvas::new(CANVAS, "layout", 100.0, 50.0)
        .widget(TestWidget::display("root").flex("Row", "Start", "Start", 0.0))
        .widget(
            TestWidget::display("a")
                .parent("root")
                .size(TestDim::Fixed(0.0), TestDim::Fixed(10.0))
                .grow(1.0)
                .background(RED),
        )
        .widget(
            TestWidget::display("b")
                .parent("root")
                .size(TestDim::Fixed(0.0), TestDim::Fixed(10.0))
                .grow(3.0)
                .background(GREEN),
        );
    let image = render(canvas)?;
    assert!(
        near(pixel(&image, 24, 5), [255, 0, 0, 255], 0),
        "the quarter of the first"
    );
    assert!(
        near(pixel(&image, 26, 5), [0, 255, 0, 255], 0),
        "the rest of the second"
    );
    assert!(
        near(pixel(&image, 99, 5), [0, 255, 0, 255], 0),
        "up to the end"
    );
    Ok(())
}

#[test]
fn fixed_child_bounds_leave_the_right_space_for_a_flex_sibling() -> TestResult {
    let image = render(
        TestCanvas::new(CANVAS, "layout", 80.0, 100.0)
            .widget(
                TestWidget::display("root")
                    .size(TestDim::Fixed(80.0), TestDim::Fixed(100.0))
                    .flex("Column", "Start", "Start", 0.0),
            )
            .widget(
                TestWidget::display("auto")
                    .parent("root")
                    .size(TestDim::Fixed(80.0), TestDim::Auto)
                    .flex("Column", "Start", "Start", 0.0),
            )
            .widget(
                TestWidget::display("capped")
                    .parent("auto")
                    .size(TestDim::Fixed(80.0), TestDim::Fixed(120.0))
                    .max_height(TestDim::Fixed(40.0))
                    .background(RED),
            )
            .widget(
                TestWidget::display("minimum")
                    .parent("auto")
                    .size(TestDim::Fixed(80.0), TestDim::Fixed(10.0))
                    .min_height(TestDim::Fixed(20.0))
                    .background(BLUE),
            )
            .widget(
                TestWidget::display("sibling")
                    .parent("root")
                    .size(TestDim::Fixed(80.0), TestDim::Fixed(40.0))
                    .shrink(1.0)
                    .background(GREEN),
            ),
    )?;
    assert_eq!(
        pixel(&image, 5, 39),
        [255, 0, 0, 255],
        "the first child ends at its maximum",
    );
    assert_eq!(
        pixel(&image, 5, 59),
        [0, 0, 255, 255],
        "the second child occupies its minimum",
    );
    assert_eq!(
        pixel(&image, 5, 60),
        [0, 255, 0, 255],
        "the sibling starts after both bounded children",
    );
    assert_eq!(
        pixel(&image, 5, 99),
        [0, 255, 0, 255],
        "the sibling keeps its full height",
    );
    Ok(())
}

#[test]
fn margin_and_padding_move_boxes_inwards() -> TestResult {
    let canvas = TestCanvas::new(CANVAS, "layout", 100.0, 50.0)
        .widget(TestWidget::display("outer").padding([10.0, 0.0, 0.0, 20.0]))
        .widget(
            TestWidget::display("inner")
                .parent("outer")
                .margin([5.0, 0.0, 0.0, 5.0])
                .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
                .background(RED),
        );
    let image = render(canvas)?;
    assert_eq!(pixel(&image, 24, 14), NOTHING, "inside padding and margin");
    assert!(
        near(pixel(&image, 26, 16), [255, 0, 0, 255], 0),
        "the box after both"
    );
    Ok(())
}

#[test]
fn a_share_of_the_height_keeps_a_box_square() -> TestResult {
    let image = render(
        TestCanvas::new(CANVAS, "layout", 100.0, 50.0).widget(
            TestWidget::display("square")
                .size(TestDim::OfHeight(1.0), TestDim::Percent(1.0))
                .background(RED),
        ),
    )?;
    assert!(
        near(pixel(&image, 49, 25), [255, 0, 0, 255], 0),
        "as wide as high"
    );
    assert_eq!(pixel(&image, 51, 25), NOTHING, "not wider");
    Ok(())
}

#[test]
fn absurd_sizes_and_offsets_are_not_drawn_and_do_not_panic() -> TestResult {
    let canvas = TestCanvas::new(CANVAS, "layout", 100.0, 50.0)
        .widget(
            TestWidget::display("huge")
                .size(TestDim::Fixed(1.0e30), TestDim::Fixed(10.0))
                .background(RED),
        )
        .widget(
            TestWidget::display("far")
                .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
                .offset(f32::NAN, 1.0e38)
                .background(GREEN),
        )
        .widget(
            TestWidget::display("negative")
                .size(TestDim::Fixed(-5.0), TestDim::Fixed(10.0))
                .background(BLUE),
        );
    let image = render(canvas)?;
    assert!(
        image
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == NOTHING),
        "nothing is drawn"
    );
    Ok(())
}

#[test]
fn a_widget_out_of_the_flow_sits_at_its_offset() -> TestResult {
    let canvas = boxes(1, "Row", "Start", 0.0).widget(
        TestWidget::display("floating")
            .parent("root")
            .size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
            .offset(60.0, 30.0)
            .out_of_flow()
            .background(BLUE),
    );
    let image = render(canvas)?;
    assert!(
        near(pixel(&image, 65, 35), [0, 0, 255, 255], 0),
        "at the offset"
    );
    assert!(
        near(pixel(&image, 5, 5), [255, 0, 0, 255], 0),
        "the flow item stays first"
    );
    Ok(())
}

#[test]
fn raw_canvases_count_fixed_values_in_pixels_of_their_size() -> TestResult {
    let image = render(
        TestCanvas::new(CANVAS, "layout", 200.0, 100.0)
            .raw()
            .widget(
                TestWidget::display("box")
                    .size(TestDim::Fixed(50.0), TestDim::Fixed(25.0))
                    .background(RED),
            ),
    )?;
    assert!(
        near(pixel(&image, 49, 24), [255, 0, 0, 255], 0),
        "50 x 25 pixels"
    );
    assert_eq!(pixel(&image, 51, 24), NOTHING, "not wider");
    assert_eq!(pixel(&image, 49, 26), NOTHING, "not higher");
    Ok(())
}

#[test]
fn a_world_canvas_fits_its_root_widget_into_the_frame_and_centres_it() -> TestResult {
    let image = render(
        TestCanvas::new(CANVAS, "layout", 200.0, 100.0)
            .raw()
            .widget(
                TestWidget::display("root")
                    .size(TestDim::Fixed(1.0), TestDim::Fixed(1.0))
                    .pivot(0.5, 0.5),
            )
            .widget(
                TestWidget::display("fill")
                    .parent("root")
                    .size(TestDim::Percent(1.0), TestDim::Percent(1.0))
                    .background(RED),
            ),
    )?;
    assert!(
        near(pixel(&image, 100, 50), [255, 0, 0, 255], 0),
        "the root square fills the height and sits in the middle"
    );
    assert!(
        near(pixel(&image, 51, 2), [255, 0, 0, 255], 0),
        "up to 50 pixels from the left"
    );
    assert_eq!(pixel(&image, 40, 50), NOTHING, "left of the root");
    assert_eq!(pixel(&image, 160, 50), NOTHING, "right of the root");
    Ok(())
}

#[test]
fn roots_of_other_sizes_do_not_make_a_pixel_canvas_a_world_canvas() -> TestResult {
    let image = render(
        TestCanvas::new(CANVAS, "layout", 200.0, 100.0)
            .raw()
            .widget(
                TestWidget::display("dot")
                    .size(TestDim::Fixed(1.0), TestDim::Fixed(1.0))
                    .background(RED),
            )
            .widget(
                TestWidget::display("panel")
                    .size(TestDim::Fixed(50.0), TestDim::Fixed(50.0))
                    .background(GREEN),
            ),
    )?;
    assert!(
        near(pixel(&image, 49, 49), [0, 255, 0, 255], 0),
        "the large root is 50 pixels"
    );
    assert_eq!(
        pixel(&image, 51, 10),
        NOTHING,
        "and not scaled to the frame"
    );
    Ok(())
}
