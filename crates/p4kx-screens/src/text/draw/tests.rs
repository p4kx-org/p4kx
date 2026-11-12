// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::super::fit::Fit;
use super::Prepared;
use crate::paint::Area;
use crate::text::{Horizontal, Vertical};

fn prepared(height_input: Option<f32>, fit: Option<Fit>) -> Prepared {
    Prepared {
        font: 0,
        size: 10.0,
        cells: Vec::new(),
        line_spacing: 0.0,
        horizontal: Horizontal::Left,
        vertical: Vertical::Top,
        wrap: false,
        height_input,
        fit,
        input: None,
    }
}

fn area(width: f32, height: f32) -> Area {
    Area {
        x: 0.0,
        y: 0.0,
        width,
        height,
    }
}

#[test]
fn a_text_with_a_size_of_its_own_holds_for_any_area() {
    assert!(prepared(None, None).holds_for(area(1.0, 2.0)));
}

#[test]
fn a_size_taken_from_the_height_holds_only_for_that_height() {
    let text = prepared(Some(20.0), None);
    assert!(text.holds_for(area(5.0, 20.0)));
    assert!(!text.holds_for(area(5.0, 21.0)));
}

#[test]
fn an_unshrunk_text_holds_for_any_width_that_still_holds_it() {
    let text = prepared(
        None,
        Some(Fit {
            width: 100.0,
            widest: 60.0,
            shrunk: false,
        }),
    );
    assert!(text.holds_for(area(60.0, 1.0)));
    assert!(text.holds_for(area(100.0, 1.0)));
    assert!(!text.holds_for(area(59.0, 1.0)));
}

#[test]
fn a_shrunk_text_holds_only_for_the_width_it_was_shrunk_to() {
    let text = prepared(
        None,
        Some(Fit {
            width: 50.0,
            widest: 80.0,
            shrunk: true,
        }),
    );
    assert!(text.holds_for(area(50.0, 1.0)));
    assert!(!text.holds_for(area(90.0, 1.0)));
    assert!(
        !text.holds_for(area(40.0, 1.0)),
        "a narrower width shrinks it more"
    );
}

#[test]
fn the_height_and_the_width_must_both_hold() {
    let text = prepared(
        Some(20.0),
        Some(Fit {
            width: 100.0,
            widest: 60.0,
            shrunk: false,
        }),
    );
    assert!(text.holds_for(area(80.0, 20.0)));
    assert!(!text.holds_for(area(80.0, 25.0)), "the height differs");
    assert!(
        !text.holds_for(area(50.0, 20.0)),
        "the width no longer holds it"
    );
    assert!(!text.holds_for(area(50.0, 25.0)), "neither holds");
}
