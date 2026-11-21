// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! State tags a canvas binds to its widgets, and the animations a style entry settles.

use p4kx_dataforge::DataForge;
use p4kx_screens::{Composed, ScreenComposer, Variable};
use p4kx_testkit::{
    TestAlphaAnimation, TestCanvas, TestCondition, TestDim, TestModifier, TestOp, TestStyleEntry,
    TestWidget, tag_record,
};

use crate::support::{
    BLUE, GREEN, RED, SCREEN, TestResult, blue_at, database, empty_at, green_at, guid_text, red_at,
    stage,
};

const ON: u128 = SCREEN + 0x200;
const OFF: u128 = SCREEN + 0x201;

fn canvas() -> TestCanvas {
    TestCanvas::new(SCREEN, "state_tags", 10.0, 10.0)
}

fn swatch() -> TestWidget {
    TestWidget::display("swatch").size(TestDim::Fixed(10.0), TestDim::Fixed(10.0))
}

fn paint(name: &str, tag: u128, colour: p4kx_testkit::TestColour) -> TestStyleEntry {
    TestStyleEntry::new(name)
        .when(TestCondition::Tag(tag))
        .set(TestModifier::EnableBackground(true))
        .set(TestModifier::Background(colour))
}

fn compose(canvas: &TestCanvas, settings: &[&str]) -> Result<Composed, Box<dyn std::error::Error>> {
    let records = vec![
        canvas.record(),
        tag_record(ON, "On"),
        tag_record(OFF, "Off"),
    ];
    let bytes = database(records);
    let forge = DataForge::open(&bytes)?;
    let variables = settings
        .iter()
        .map(|setting| Ok(setting.parse::<Variable>()?))
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    Ok(ScreenComposer::new(&forge)
        .with_variables(&variables)
        .compose(&guid_text(SCREEN), None)?)
}

fn bound() -> TestCanvas {
    canvas()
        .embedded(paint("on", ON, RED))
        .embedded(paint("off", OFF, GREEN))
        .operation("flag", TestOp::BoolVar)
        .operation("pick", TestOp::TagFromBool("flag", ON, OFF))
        .widget(swatch())
        .operation(
            "bind",
            TestOp::TagField("swatch", "PrimaryStateTag", "pick"),
        )
}

#[test]
fn a_state_tag_selects_the_style_entry_that_colours_the_widget() -> TestResult {
    let at_rest = compose(&bound(), &[])?;
    assert!(green_at(&at_rest.rasterize(&stage())?.image, 5, 5));
    let on = compose(&bound(), &["variable=true"])?;
    assert!(red_at(&on.rasterize(&stage())?.image, 5, 5));
    Ok(())
}

#[test]
fn a_parent_background_stays_at_rest_and_only_the_authored_style_hides_it() -> TestResult {
    let hide = TestStyleEntry::new("hide background")
        .when(TestCondition::Tag(ON))
        .set(TestModifier::EnableBackground(false));
    let chip = canvas()
        .embedded(hide)
        .operation("flag", TestOp::BoolVar)
        .operation("pick", TestOp::TagFromBool("flag", ON, OFF))
        .widget(swatch().background(BLUE))
        .widget(
            TestWidget::display("label")
                .parent("swatch")
                .size(TestDim::Fixed(4.0), TestDim::Fixed(4.0))
                .background(GREEN),
        )
        .operation(
            "bind",
            TestOp::TagField("swatch", "PrimaryStateTag", "pick"),
        );
    let at_rest = compose(&chip, &[])?.rasterize(&stage())?.image;
    let changed = compose(&chip, &["variable=true"])?
        .rasterize(&stage())?
        .image;
    assert!(
        blue_at(&at_rest, 8, 8),
        "the parent background draws at rest"
    );
    assert!(
        empty_at(&changed, 8, 8),
        "the active style hides the parent background"
    );
    assert!(green_at(&at_rest, 1, 1), "the child draws at rest");
    assert!(
        green_at(&changed, 1, 1),
        "the parent style leaves the child visible"
    );
    Ok(())
}

#[test]
fn a_state_tag_is_not_listed_as_left_out() -> TestResult {
    let composed = compose(&bound(), &[])?;
    let listed = composed
        .report()
        .unsupported
        .iter()
        .any(|entry| entry.element == "Binding" && entry.reason.contains("state tag"));
    assert!(!listed, "{:?}", composed.report().unsupported);
    Ok(())
}

fn animated(animation: TestAlphaAnimation) -> TestCanvas {
    canvas()
        .embedded(paint("shown", ON, BLUE).set(TestModifier::Animation(animation)))
        .operation("flag", TestOp::BoolVar)
        .operation("pick", TestOp::TagFromBool("flag", ON, OFF))
        .widget(swatch())
        .operation(
            "bind",
            TestOp::TagField("swatch", "PrimaryStateTag", "pick"),
        )
}

#[test]
fn an_animation_that_holds_its_last_frame_settles_on_it() -> TestResult {
    let settled = compose(
        &animated(TestAlphaAnimation::alpha("Forward", "Forward", 0.0, 1.0)),
        &["variable=true"],
    )?;
    assert!(
        blue_at(&settled.rasterize(&stage())?.image, 5, 5),
        "from nothing to opaque: opaque"
    );
    let faded = compose(
        &animated(TestAlphaAnimation::alpha("Forward", "Forward", 1.0, 0.0)),
        &["variable=true"],
    )?;
    assert!(
        empty_at(&faded.rasterize(&stage())?.image, 5, 5),
        "from opaque to nothing: nothing"
    );
    let reversed = compose(
        &animated(TestAlphaAnimation::alpha("Both", "Reverse", 1.0, 0.0)),
        &["variable=true"],
    )?;
    assert!(
        blue_at(&reversed.rasterize(&stage())?.image, 5, 5),
        "a reverse animation settles on its first frame"
    );
    Ok(())
}

/// Composes a screen whose animation would fade the swatch out if it settled, and returns whether
/// the swatch stays opaque.
fn stays_opaque(animation: TestAlphaAnimation) -> Result<bool, Box<dyn std::error::Error>> {
    let composed = compose(&animated(animation), &["variable=true"])?;
    Ok(blue_at(&composed.rasterize(&stage())?.image, 5, 5))
}

#[test]
fn an_animation_that_does_not_settle_changes_nothing() -> TestResult {
    let fade = TestAlphaAnimation::alpha("Forward", "Forward", 1.0, 0.0);
    assert!(
        stays_opaque(TestAlphaAnimation::alpha("None", "Forward", 1.0, 0.0))?,
        "the frame is not held"
    );
    assert!(stays_opaque(fade.looping())?, "it loops");
    assert!(
        stays_opaque(TestAlphaAnimation::alpha("Forward", "Alternate", 1.0, 0.0))?,
        "it alternates"
    );
    assert!(
        stays_opaque(fade.without_keyframes())?,
        "it has no keyframes"
    );
    assert!(
        stays_opaque(fade.ending_at(0.5))?,
        "its last frame is not at the end of the timeline"
    );
    Ok(())
}
