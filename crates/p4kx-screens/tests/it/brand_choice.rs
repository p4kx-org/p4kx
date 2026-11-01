// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The brand style of a canvas: the one of the requested brand, the first without a request, none
//! for a brand the canvas does not style.

use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_screens::ScreenComposer;
use p4kx_testkit::{
    TestCanvas, TestColour, TestDim, TestModifier, TestStyle, TestStyleEntry, TestWidget,
};

use crate::support::{TestResult, database, guid_text, near, pixel, stage};

const CANVAS: u128 = 0x5b00_0000_0000_4000_8000_0000_0000_0002;
const STYLE: u128 = 0x5b00_0000_0000_4000_8000_0000_0000_0003;
const GREEN: [u8; 4] = [0, 255, 0, 255];
const RED: [u8; 4] = [255, 0, 0, 255];

/// A canvas whose default entry fills the root widget red and whose only brand style,
/// `s_test_hud`, fills it green.
fn canvas() -> TestCanvas {
    let fill = TestStyleEntry::new("fill")
        .set(TestModifier::EnableBackground(true))
        .set(TestModifier::Background(TestColour::Solid(GREEN)));
    let default = TestStyleEntry::new("default fill")
        .set(TestModifier::EnableBackground(true))
        .set(TestModifier::Background(TestColour::Solid(RED)));
    TestCanvas::new(CANVAS, "brand choice", 10.0, 10.0)
        .default_style(default)
        .brand_style(STYLE, vec![fill])
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
