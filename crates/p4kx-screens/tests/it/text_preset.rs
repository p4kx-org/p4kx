// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A text field takes the font size of its text preset from the canvas of the standard text field.

use p4kx_dataforge::DataForge;
use p4kx_screens::{ScreenComposer, TextSources};
use p4kx_testkit::{
    SCREEN_TEXT_STYLE, TestCanvas, TestColour, TestCondition, TestDim, TestInstance, TestModifier,
    TestStyle, TestStyleEntry, TestValue, TestWidget, named_tag_record, screens_text_sample,
};

use crate::support::{SCREEN, TestResult, guid_text, pixel, stage};
use crate::text::fonts;

const KIT_FOLDER: &str = "libs/foundry/records/ui/buildingblocks/modularkit/standard/widgets/";
const TAG_FOLDER: &str = "UI.Style._Standard.ModularKit.Widgets.textfield.";
const STANDARD: u128 = SCREEN + 0x500;
const HEADING1: u128 = SCREEN + 0x501;
const BRAND: u128 = SCREEN + 0x502;
const HEADING2: u128 = SCREEN + 0x503;

/// The default font draws an `A` 6000 units wide and 14000 tall on an em of 20480 units, with its
/// baseline 16000 units below the top of the line.
const DEFAULT_SIZE: f32 = 16.0;
const BRAND_SIZE: f32 = 8.0;

fn field(preset: &'static str) -> TestWidget {
    TestWidget::new("BuildingBlocks_WidgetTextField", "text")
        .parent("root")
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

fn size_entry(tag: u128, size: f32) -> TestStyleEntry {
    TestStyleEntry::new("H1")
        .when(TestCondition::Tag(tag))
        .set(TestModifier::FontSize(size))
}

fn standard() -> TestCanvas {
    TestCanvas::new(STANDARD, "textfieldwidgetstandard", 1920.0, 1080.0)
        .in_folder(KIT_FOLDER)
        .default_style(size_entry(HEADING1, DEFAULT_SIZE))
        .default_style(
            TestStyleEntry::new("H2")
                .when(TestCondition::Tag(HEADING2))
                .set(TestModifier::FontStyle(SCREEN_TEXT_STYLE))
                .set(TestModifier::FontSize(DEFAULT_SIZE)),
        )
        .brand_style(BRAND, vec![size_entry(HEADING1, BRAND_SIZE)])
}

fn screen(preset: &'static str, branded: bool) -> TestCanvas {
    let ink = TestStyleEntry::new("ink")
        .when(TestCondition::Type("TextField"))
        .set(TestModifier::Fill(TestColour::Solid([255, 0, 0, 255])));
    let canvas = TestCanvas::new(SCREEN, "text_preset", 60.0, 20.0)
        .embedded(ink)
        .widget(TestWidget::display("root"))
        .widget(field(preset));
    if branded {
        canvas.brand_style(BRAND, Vec::new())
    } else {
        canvas
    }
}

fn compose(
    canvas: &TestCanvas,
    kit: bool,
) -> Result<p4kx_screens::Composed, Box<dyn std::error::Error>> {
    let mut database = screens_text_sample()
        .record(canvas.record())
        .record(TestStyle::new(BRAND, "s_test_hud", Vec::new()).record());
    if kit {
        database = database
            .record(standard().record())
            .record(named_tag_record(
                HEADING1,
                &format!("{TAG_FOLDER}textstyle-heading1"),
            ))
            .record(named_tag_record(
                HEADING2,
                &format!("{TAG_FOLDER}textstyle-heading2"),
            ));
    }
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let fonts = fonts()?;
    Ok(ScreenComposer::new(&forge)
        .with_texts(TextSources {
            fonts: &fonts,
            localise: None,
        })
        .compose(&guid_text(SCREEN), None)?)
}

fn alpha(image: &p4kx_gfx::RgbaImage, x: u32, y: u32) -> u8 {
    pixel(image, x, y)[3]
}

#[test]
fn a_preset_takes_the_default_size_of_the_standard_text_field() -> TestResult {
    let image = compose(&screen("Heading1", false), true)?
        .rasterize(&stage())?
        .image;
    assert!(alpha(&image, 4, 8) > 128, "the A reaches column 4");
    assert_eq!(alpha(&image, 6, 8), 0, "and ends before column 6");
    Ok(())
}

#[test]
fn a_preset_takes_the_font_of_the_standard_text_field() -> TestResult {
    let image = compose(&screen("Heading2", false), true)?
        .rasterize(&stage())?
        .image;
    assert!(alpha(&image, 7, 8) > 128, "the A of Test Sans is wider");
    Ok(())
}

#[test]
fn a_brand_that_sets_a_size_overrides_the_default() -> TestResult {
    let image = compose(&screen("Heading1", true), true)?
        .rasterize(&stage())?
        .image;
    assert!(alpha(&image, 1, 3) > 128, "the small A is drawn");
    assert_eq!(alpha(&image, 4, 3), 0, "it is narrower than the default");
    assert_eq!(alpha(&image, 1, 9), 0, "and lower than the default");
    Ok(())
}

#[test]
fn a_size_of_the_cascade_wins_over_the_preset() -> TestResult {
    let own = TestStyleEntry::new("own")
        .when(TestCondition::Type("TextField"))
        .set(TestModifier::FontSize(BRAND_SIZE));
    let canvas = screen("Heading1", false).embedded(own);
    let image = compose(&canvas, true)?.rasterize(&stage())?.image;
    assert_eq!(alpha(&image, 4, 3), 0, "the A has the size of the entry");
    Ok(())
}

#[test]
fn a_preset_the_archive_sets_no_size_for_is_listed_and_sized_by_its_box() -> TestResult {
    let composed = compose(&screen("Title1", false), true)?;
    let listed = composed.report().unsupported.iter().any(|entry| {
        entry
            .reason
            .contains("text style Title1 has no font size in the Archive")
    });
    assert!(listed, "{:?}", composed.report().unsupported);
    Ok(())
}

#[test]
fn without_the_standard_text_field_a_preset_is_listed() -> TestResult {
    let composed = compose(&screen("Heading1", false), false)?;
    let listed = composed.report().unsupported.iter().any(|entry| {
        entry
            .reason
            .contains("text style Heading1 has no font size in the Archive")
    });
    assert!(listed, "{:?}", composed.report().unsupported);
    Ok(())
}
