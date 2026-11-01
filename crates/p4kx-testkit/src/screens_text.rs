// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::screens_canvas::{TestCanvas, TestCondition, TestModifier, TestStyleEntry, tag_record};
use crate::screens_widget::{TestColour, TestDim, TestWidget};
use crate::{
    DataForgeBuilder, TestGlyph, TestMovie, TestRecord, TestShapeRecord, TestSignature, TestValue,
    entity_schema, font3_body_styled, screens_schema,
};

/// The GUID of the synthetic canvas [`screens_text_sample`] holds, 200 x 100 pixels.
pub const SCREEN_TEXT_CANVAS: u128 = 0x4e71_a0c5_83d2_4f19_9b36_d7e8_12a4_c05b;

/// The GUID of the font style Record `testsans`, whose font `Test Sans` is in
/// [`screens_font_movie`].
pub const SCREEN_TEXT_STYLE: u128 = 0x93b0_2d6e_f1a7_48c4_a5e9_0c37_b8d1_6f42;

/// The GUID of the font style Record `blenderpromedium`, whose font `Blender Pro Medium` is the
/// default font of [`screens_font_movie`].
pub const SCREEN_DEFAULT_STYLE: u128 = 0x6c1e_95a3_0d47_4b82_91fa_e3b8_5720_c4d6;

const SCREEN_TEXT_TAG: u128 = 0xd2f4_7a18_6b93_4e0d_8c51_a9e3_f720_14b6;
const FONT_STYLES: &str = "libs/foundry/records/ui/buildingblocks/fontstyles/";
const RED: TestColour = TestColour::Solid([255, 0, 0, 255]);
const GREEN: TestColour = TestColour::Solid([0, 255, 0, 255]);
const ASCENT: u16 = 16_000;
const DESCENT: u16 = 4_000;

fn glyph(code: char, advance: i16, width: i32, height: i32) -> TestGlyph {
    let code = u16::try_from(u32::from(code)).unwrap_or(0);
    if width == 0 {
        return TestGlyph {
            code,
            advance,
            records: Vec::new(),
        };
    }
    TestGlyph {
        code,
        advance,
        records: vec![
            TestShapeRecord::Move {
                to: (0, 0),
                fill0: 0,
                fill1: 1,
                line: 0,
            },
            TestShapeRecord::Line(width, 0),
            TestShapeRecord::Line(0, -height),
            TestShapeRecord::Line(-width, 0),
            TestShapeRecord::Line(0, height),
        ],
    }
}

/// Returns a GFx movie with the fonts of the synthetic screens.
///
/// `Blender Pro Medium`, the default font, has an `A` of 6000 x 14000 units, and `Test Sans`, whose `A` and `B` are
/// 10000 x 14000 and 10000 x 7000 boxes and whose space has no outline. The em square is
/// 20480 units, the ascent 16000 and the descent 4000, every advance 12000 (space 6000).
#[must_use]
pub fn screens_font_movie() -> Vec<u8> {
    let layout = (false, ASCENT, DESCENT);
    let default = [glyph('A', 12_000, 6_000, 14_000), glyph(' ', 6_000, 0, 0)];
    let sans = [
        glyph('A', 12_000, 10_000, 14_000),
        glyph('B', 12_000, 10_000, 7_000),
        glyph(' ', 6_000, 0, 0),
    ];
    TestMovie::new(TestSignature::Gfx)
        .stage(200, 100)
        .tag(
            75,
            &font3_body_styled(1, "Blender Pro Medium", &default, layout),
        )
        .tag(75, &font3_body_styled(2, "Test Sans", &sans, layout))
        .bytes()
}

/// Looks the localisation keys of the synthetic screens up: `test_hello` is `AB`, `test_pair`
/// is `A\nB`.
#[must_use]
pub fn text_localisation(key: &str) -> Option<String> {
    match key {
        "test_hello" => Some("AB".to_owned()),
        "test_pair" => Some("A\nB".to_owned()),
        _ => None,
    }
}

fn font_style() -> TestRecord {
    TestRecord::new(
        SCREEN_TEXT_STYLE,
        "BuildingBlocks_FontStyle",
        "BuildingBlocks_FontStyle.testsans",
        format!("{FONT_STYLES}testsans.xml"),
    )
    .value("font", TestValue::text("$TestSans"))
    .value(
        "paintFile",
        TestValue::text("UI/fonts/Install/TestSans-Regular.slug"),
    )
    .value("isBold", TestValue::Bool(false))
    .value("scaleModifier", TestValue::Single(1.0))
}

fn default_font_style() -> TestRecord {
    TestRecord::new(
        SCREEN_DEFAULT_STYLE,
        "BuildingBlocks_FontStyle",
        "BuildingBlocks_FontStyle.blenderpromedium",
        format!("{FONT_STYLES}blenderpromedium.xml"),
    )
    .value("font", TestValue::text("$Text1Med"))
    .value(
        "paintFile",
        TestValue::text("UI/fonts/Install/BlenderPro-Medium.slug"),
    )
    .value("isBold", TestValue::Bool(false))
    .value("scaleModifier", TestValue::Single(1.0))
}

fn text(name: &'static str, source: &'static str, at: (f32, f32), size: (f32, f32)) -> TestWidget {
    TestWidget::new("BuildingBlocks_WidgetText", name)
        .parent("root")
        .size(TestDim::Fixed(size.0), TestDim::Fixed(size.1))
        .offset(at.0, at.1)
        .with("locString", TestValue::text(source))
        .with("fontStyle", TestValue::Reference(SCREEN_TEXT_STYLE))
        .with("fontSize", TestValue::Single(10.0))
        .with("color", RED.value())
}

fn aligned(widget: TestWidget, horizontal: &'static str) -> TestWidget {
    widget.with("textAlignment", TestValue::text(horizontal))
}

fn canvas() -> TestCanvas {
    let styled = TestStyleEntry::new("styled")
        .when(TestCondition::Tag(SCREEN_TEXT_TAG))
        .set(TestModifier::FontSize(20.0))
        .set(TestModifier::Fill(GREEN))
        .set(TestModifier::Horizontal("Right"));
    let key = TestWidget::new("BuildingBlocks_WidgetText", "key")
        .parent("root")
        .size(TestDim::Fixed(100.0), TestDim::Fixed(20.0))
        .offset(0.0, 60.0)
        .with("locString", TestValue::text("@test_hello"))
        .with("fontStyle", TestValue::Reference(SCREEN_TEXT_STYLE))
        .with("fontSize", TestValue::Single(10.0))
        .with("color", RED.value());
    [
        TestWidget::display("root"),
        text("left", "AB", (0.0, 0.0), (100.0, 20.0)),
        aligned(text("right", "A", (0.0, 20.0), (100.0, 20.0)), "Right"),
        aligned(text("centre", "A", (0.0, 40.0), (100.0, 20.0)), "Center"),
        text("wrapped", "AA AA", (100.0, 0.0), (20.0, 40.0))
            .with("wordWrap", TestValue::Bool(true)),
        key,
        text("missing", "Z", (100.0, 50.0), (20.0, 20.0)),
        text("styled", "A", (0.0, 80.0), (100.0, 20.0))
            .tag(SCREEN_TEXT_TAG)
            .with("fontSize", TestValue::Single(10.0)),
        TestWidget::display("row")
            .parent("root")
            .size(TestDim::Fixed(60.0), TestDim::Fixed(20.0))
            .offset(130.0, 80.0)
            .flex("Row", "Start", "Start", 0.0),
        TestWidget::new("BuildingBlocks_WidgetText", "auto")
            .parent("row")
            .size(TestDim::Auto, TestDim::Fixed(20.0))
            .with("locString", TestValue::text("AB"))
            .with("fontStyle", TestValue::Reference(SCREEN_TEXT_STYLE))
            .with("fontSize", TestValue::Single(10.0))
            .with("color", RED.value()),
        TestWidget::display("after")
            .parent("row")
            .size(TestDim::Fixed(6.0), TestDim::Fixed(20.0))
            .background(GREEN),
    ]
    .into_iter()
    .fold(
        TestCanvas::new(SCREEN_TEXT_CANVAS, "text_sample", 200.0, 100.0).embedded(styled),
        TestCanvas::widget,
    )
}

/// Returns a synthetic DataForge with the text canvas [`SCREEN_TEXT_CANVAS`], 200 x 100 pixels.
///
/// Its text widgets use the font `Test Sans` of [`screens_font_movie`]: left, right and centre aligned
/// texts, a wrapping one, a localisation key, a character without a glyph, a text whose size,
/// colour and alignment a style entry sets, and a text of automatic width in a row.
#[must_use]
pub fn screens_text_sample() -> DataForgeBuilder {
    screens_schema(entity_schema(DataForgeBuilder::new()))
        .record(font_style())
        .record(default_font_style())
        .record(tag_record(SCREEN_TEXT_TAG, "Styled"))
        .record(canvas().record())
}
