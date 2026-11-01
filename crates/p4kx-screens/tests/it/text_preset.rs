// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A text field takes the font size of its text preset from the canvas of the standard text field,
//! and its font where the size comes from there too.

use p4kx_dataforge::DataForge;
use p4kx_gfx::RgbaImage;
use p4kx_screens::{Composed, ScreenComposer, TextSources};
use p4kx_testkit::{
    SCREEN_DEFAULT_STYLE, SCREEN_TEXT_STYLE, TestCanvas, TestColour, TestCondition, TestDim,
    TestInstance, TestModifier, TestStyle, TestStyleEntry, TestValue, TestWidget, clone_modifiers,
    named_tag_record, screens_text_sample,
};

use crate::support::{SCREEN, TestResult, guid_text, pixel, stage};
use crate::text::fonts;

const KIT_FOLDER: &str = "libs/foundry/records/ui/buildingblocks/modularkit/standard/widgets/";
const TAG_FOLDER: &str = "UI.Style._Standard.ModularKit.Widgets.textfield.";
const STANDARD: u128 = SCREEN + 0x500;
const HEADING1: u128 = SCREEN + 0x501;
const BRAND: u128 = SCREEN + 0x502;
const HEADING2: u128 = SCREEN + 0x503;
const HEADING3: u128 = SCREEN + 0x504;
const OTHER_BRAND: u128 = SCREEN + 0x505;

/// The default font draws an `A` 6000 units wide and 14000 tall on an em of 20480 units, with its
/// baseline 16000 units below the top of the line; `Test Sans` draws it 10000 units wide.
const DEFAULT_SIZE: f32 = 16.0;
const BRAND_SIZE: f32 = 8.0;

fn field(preset: &'static str) -> TestWidget {
    TestWidget::new("BuildingBlocks_WidgetTextField", "text")
        .size(TestDim::Fixed(20.0), TestDim::Fixed(20.0))
        .with(
            "labelProperties",
            TestValue::Struct(
                TestInstance::new("BuildingBlocks_ComponentLabelProperties")
                    .value("label", TestValue::text("A"))
                    .value("style", TestValue::text(preset)),
            ),
        )
}

fn preset_entry(tag: u128, font: Option<u128>, size: f32) -> TestStyleEntry {
    let entry = TestStyleEntry::new("preset")
        .when(TestCondition::Tag(tag))
        .set(TestModifier::FontSize(size));
    match font {
        Some(font) => entry.set(TestModifier::FontStyle(font)),
        None => entry,
    }
}

fn standard() -> TestCanvas {
    TestCanvas::new(STANDARD, "textfieldwidgetstandard", 1920.0, 1080.0)
        .in_folder(KIT_FOLDER)
        .default_style(preset_entry(HEADING1, None, DEFAULT_SIZE))
        .default_style(preset_entry(
            HEADING2,
            Some(SCREEN_TEXT_STYLE),
            DEFAULT_SIZE,
        ))
        .default_style(preset_entry(
            HEADING3,
            Some(SCREEN_DEFAULT_STYLE),
            DEFAULT_SIZE,
        ))
        .brand_style(BRAND, vec![preset_entry(HEADING1, None, BRAND_SIZE)])
}

fn ink() -> TestStyleEntry {
    TestStyleEntry::new("ink")
        .when(TestCondition::Type("TextField"))
        .set(TestModifier::Fill(TestColour::Solid([255, 0, 0, 255])))
}

/// A screen with a text field of the preset in a 60 x 20 canvas that has no brand style.
fn screen(preset: &'static str) -> TestCanvas {
    TestCanvas::new(SCREEN, "text_preset", 60.0, 20.0)
        .embedded(ink())
        .widget(TestWidget::display("root"))
        .widget(field(preset).parent("root"))
}

fn compose(
    canvas: &TestCanvas,
    kit: bool,
    brand: Option<&str>,
) -> Result<Composed, Box<dyn std::error::Error>> {
    let mut database = screens_text_sample()
        .record(canvas.record())
        .record(TestStyle::new(BRAND, "s_test_hud", Vec::new()).record())
        .record(TestStyle::new(OTHER_BRAND, "s_other_hud", Vec::new()).record());
    if kit {
        for (tag, name) in [
            (HEADING1, "heading1"),
            (HEADING2, "heading2"),
            (HEADING3, "heading3"),
        ] {
            database = database.record(named_tag_record(
                tag,
                &format!("{TAG_FOLDER}textstyle-{name}"),
            ));
        }
        database = database.record(standard().record());
    }
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let fonts = fonts()?;
    Ok(ScreenComposer::new(&forge)
        .with_texts(TextSources {
            fonts: &fonts,
            localise: None,
        })
        .compose(&guid_text(SCREEN), brand)?)
}

fn render(
    canvas: &TestCanvas,
    brand: Option<&str>,
) -> Result<RgbaImage, Box<dyn std::error::Error>> {
    Ok(compose(canvas, true, brand)?.rasterize(&stage())?.image)
}

fn alpha(image: &RgbaImage, x: u32, y: u32) -> u8 {
    pixel(image, x, y)[3]
}

/// Asserts the `A` of size 16 in the default font: it reaches column 4 and ends before column 6.
fn assert_default_size(image: &RgbaImage) {
    assert!(alpha(image, 4, 8) > 128, "the A reaches column 4");
    assert_eq!(alpha(image, 6, 8), 0, "and ends before column 6");
}

/// Asserts the `A` of size 8: it is narrower and lower than the one of size 16.
fn assert_small_size(image: &RgbaImage) {
    assert!(alpha(image, 1, 3) > 128, "the small A is drawn");
    assert_eq!(alpha(image, 4, 3), 0, "it is narrower than the default");
    assert_eq!(alpha(image, 1, 9), 0, "and lower than the default");
}

#[test]
fn a_preset_takes_the_default_size_of_the_standard_text_field() -> TestResult {
    assert_default_size(&render(&screen("Heading1"), None)?);
    Ok(())
}

#[test]
fn a_brand_without_a_style_in_the_screen_canvas_still_gets_its_entry() -> TestResult {
    assert_small_size(&render(&screen("Heading1"), Some("test"))?);
    Ok(())
}

#[test]
fn a_brand_the_standard_text_field_lacks_gets_the_default() -> TestResult {
    assert_default_size(&render(&screen("Heading1"), Some("other"))?);
    Ok(())
}

#[test]
fn a_preset_takes_the_font_where_it_takes_the_size() -> TestResult {
    let image = render(&screen("Heading2"), None)?;
    assert!(alpha(&image, 7, 8) > 128, "the A of Test Sans is wider");
    Ok(())
}

#[test]
fn a_size_of_the_cascade_wins_and_leaves_the_font_as_it_was() -> TestResult {
    let own = TestStyleEntry::new("own")
        .when(TestCondition::Type("TextField"))
        .set(TestModifier::FontSize(BRAND_SIZE));
    let image = render(&screen("Heading2").embedded(own), None)?;
    assert_eq!(alpha(&image, 4, 3), 0, "the A has the size of the entry");
    assert!(alpha(&image, 1, 3) > 128, "in the default font");
    assert_eq!(
        alpha(&image, 3, 3),
        0,
        "not in the wider font of the preset"
    );
    Ok(())
}

#[test]
fn an_inline_size_wins_over_the_preset() -> TestResult {
    let own = TestStyleEntry::new("inline").set(TestModifier::FontSize(BRAND_SIZE));
    let canvas = TestCanvas::new(SCREEN, "text_preset", 60.0, 20.0)
        .embedded(ink())
        .widget(TestWidget::display("root"))
        .widget(field("Heading1").parent("root").inline_style(&own));
    assert_small_size(&render(&canvas, None)?);
    Ok(())
}

#[test]
fn the_size_an_instance_sets_wins_over_the_preset() -> TestResult {
    let canvas = TestCanvas::new(SCREEN, "text_preset", 60.0, 20.0)
        .embedded(ink())
        .template(field("Heading1"))
        .widget(TestWidget::display("root"))
        .widget(TestWidget::clone_of("copy", "text").parent("root").with(
            "modifiers",
            clone_modifiers(&[TestModifier::FontSize(BRAND_SIZE)]),
        ));
    assert_small_size(&render(&canvas, None)?);
    Ok(())
}

#[test]
fn a_font_of_the_cascade_wins_over_the_font_of_the_preset() -> TestResult {
    let own = TestStyleEntry::new("own")
        .when(TestCondition::Type("TextField"))
        .set(TestModifier::FontStyle(SCREEN_TEXT_STYLE));
    let image = render(&screen("Heading3").embedded(own), None)?;
    assert!(
        alpha(&image, 7, 8) > 128,
        "Test Sans, not the font of the preset"
    );
    Ok(())
}

#[test]
fn a_preset_without_a_tag_is_listed_and_sized_by_its_box() -> TestResult {
    let composed = compose(&screen("Title1"), true, None)?;
    let listed = composed.report().unsupported.iter().any(|entry| {
        entry
            .reason
            .contains("text style Title1 has no font size in the Archive")
    });
    assert!(listed, "{:?}", composed.report().unsupported);
    let image = composed.rasterize(&stage())?.image;
    assert!(
        alpha(&image, 2, 5) > 128,
        "60 % of the box of 20 is a size of 12"
    );
    assert_eq!(alpha(&image, 4, 5), 0, "narrower than the size 16");
    assert_eq!(alpha(&image, 1, 10), 0, "and lower than it");
    Ok(())
}

#[test]
fn without_the_standard_text_field_that_is_listed() -> TestResult {
    let composed = compose(&screen("Heading1"), false, None)?;
    let listed = composed
        .report()
        .unsupported
        .iter()
        .any(|entry| entry.reason.contains("standard text field not found"));
    assert!(listed, "{:?}", composed.report().unsupported);
    Ok(())
}
