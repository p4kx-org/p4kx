// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The brand style of a canvas: the one of the requested brand, the first without a request, none
//! for a brand the canvas does not style.

use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_screens::{ScreenComposer, Variable};
use p4kx_testkit::{
    TestCanvas, TestColour, TestCondition, TestDim, TestModifier, TestOp, TestStyle,
    TestStyleEntry, TestWidget,
};

use crate::support::{TestResult, database, guid_text, near, pixel, stage};

const CANVAS: u128 = 0x5b00_0000_0000_4000_8000_0000_0000_0002;
const STYLE: u128 = 0x5b00_0000_0000_4000_8000_0000_0000_0003;
const SHARED: u128 = STYLE + 1;
const ALT_STYLE: u128 = STYLE + 2;
const ALT_SHARED: u128 = STYLE + 3;
const NESTED: u128 = STYLE + 4;
const BLUE: [u8; 4] = [0, 0, 255, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const MAGENTA: [u8; 4] = [255, 0, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];

/// A canvas whose default entry fills the root widget red and whose only brand style,
/// `s_test_hud`, fills it green.
fn canvas() -> TestCanvas {
    TestCanvas::new(CANVAS, "brand choice", 10.0, 10.0)
        .default_style(fill("default fill", RED))
        .brand_style(STYLE, vec![fill("fill", GREEN)])
        .widget(TestWidget::display("root").size(TestDim::Fixed(10.0), TestDim::Fixed(10.0)))
}

fn centre(brand: Option<&str>) -> Result<[u8; 4], Box<dyn Error>> {
    let style = TestStyle::new(
        STYLE,
        "s_test_hud",
        vec![TestColour::Solid([0, 0, 255, 255])],
    );
    let bytes = database([canvas().record(), style.record()]);
    let forge = DataForge::open(&bytes)?;
    let composed = ScreenComposer::new(&forge).compose(&guid_text(CANVAS), brand)?;
    Ok(pixel(&composed.rasterize(&stage())?.image, 5, 5))
}

#[test]
fn the_brand_style_of_the_requested_brand_applies() -> TestResult {
    assert!(near(centre(Some("test"))?, GREEN, 0));
    Ok(())
}

#[test]
fn without_a_requested_brand_the_first_brand_style_applies() -> TestResult {
    assert!(near(centre(None)?, GREEN, 0));
    Ok(())
}

#[test]
fn a_requested_brand_the_canvas_does_not_style_gets_the_default_entries_only() -> TestResult {
    assert!(near(centre(Some("other"))?, RED, 0));
    Ok(())
}

fn fill(name: &str, colour: [u8; 4]) -> TestStyleEntry {
    TestStyleEntry::new(name)
        .set(TestModifier::EnableBackground(true))
        .set(TestModifier::Background(TestColour::Solid(colour)))
}

fn shared_styles(local: Vec<TestStyleEntry>) -> Vec<u8> {
    let outer = TestCanvas::new(CANVAS, "brand shared styles", 10.0, 10.0)
        .widget(TestWidget::display("swatch").size(TestDim::Fixed(10.0), TestDim::Fixed(10.0)))
        .default_style(fill("default", RED))
        .brand_group(STYLE, Some(SHARED), local)
        .brand_group(ALT_STYLE, Some(ALT_SHARED), Vec::new());
    database([
        outer.record(),
        TestStyle::new(STYLE, "s_test_hud", Vec::new())
            .entry(fill("brand identifier", BLUE))
            .record(),
        TestStyle::new(SHARED, "test_component_sheet", Vec::new())
            .entry(fill("first shared style", GREEN))
            .record(),
        TestStyle::new(ALT_STYLE, "s_alt_hud", Vec::new()).record(),
        TestStyle::new(ALT_SHARED, "alt_component_sheet", Vec::new())
            .entry(fill("second shared style", YELLOW))
            .record(),
    ])
}

#[test]
fn only_the_selected_brands_shared_style_is_applied() -> TestResult {
    let bytes = shared_styles(Vec::new());
    let forge = DataForge::open(&bytes)?;
    let mut composer = ScreenComposer::new(&forge);
    for (brand, colour) in [
        (None, GREEN),
        (Some("test"), GREEN),
        (Some("alt"), YELLOW),
        (Some("unknown"), RED),
    ] {
        let screen = composer.compose(&guid_text(CANVAS), brand)?;
        let actual = pixel(&screen.rasterize(&stage())?.image, 5, 5);
        assert_eq!(actual, colour, "background with brand {brand:?}");
    }
    Ok(())
}

#[test]
fn brand_local_entries_override_the_shared_style() -> TestResult {
    let bytes = shared_styles(vec![fill("local", MAGENTA)]);
    let forge = DataForge::open(&bytes)?;
    let screen = ScreenComposer::new(&forge).compose(&guid_text(CANVAS), Some("test"))?;
    assert_eq!(
        pixel(&screen.rasterize(&stage())?.image, 5, 5),
        MAGENTA,
        "the local entry takes precedence over the shared style"
    );
    Ok(())
}

#[test]
fn the_render_survey_follows_canvas_choices_in_the_selected_brands_shared_style() -> TestResult {
    let root = TestCanvas::new(CANVAS, "brand survey", 10.0, 10.0)
        .widget(TestWidget::display("root"))
        .brand_group(STYLE, Some(SHARED), Vec::new());
    let entry = TestStyleEntry::new("alternate canvas")
        .when(TestCondition::Type("Canvas"))
        .set(TestModifier::Canvas(NESTED));
    let nested = TestCanvas::new(NESTED, "brand survey target", 10.0, 10.0)
        .operation("read switch", TestOp::NamedBoolVar("deep"));
    let bytes = database([
        root.record(),
        TestStyle::new(STYLE, "s_test_hud", Vec::new()).record(),
        TestStyle::new(SHARED, "test_component_sheet", Vec::new())
            .entry(entry)
            .record(),
        nested.record(),
    ]);
    let forge = DataForge::open(&bytes)?;
    let mut composer = ScreenComposer::new(&forge).with_variables(&[Variable::new("deep", "1")?]);
    let screen = composer.compose(&guid_text(CANVAS), Some("test"))?;
    assert_eq!(screen.report().canvases, 1, "only the root draws");
    assert_eq!(
        screen.report().unknown_variables().next(),
        None,
        "the selected shared style exposes the nested canvas to the survey"
    );
    let screen = composer.compose(&guid_text(CANVAS), Some("unknown"))?;
    assert_eq!(
        screen.report().unknown_variables().next(),
        Some("deep"),
        "an unknown brand does not survey the unused style"
    );
    Ok(())
}
