// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Static texts and text fields: glyph outlines of the embedded fonts drawn in both backends,
//! sizes, colours, alignment, markup, localisation and what is reported when fonts or glyphs
//! are missing.

use std::collections::HashMap;

use crate::support::{CLEAR, assert_pixel, pixel, stage};
use p4kx_gfx::{
    Background, CanvasSize, Movie, Raster, RasterOptions, RgbaImage, Scene, SkipReason, TextOptions,
};
use p4kx_testkit::{
    TestEditText, TestGlyph, TestPlace, TestShapeRecord, TestTextRecord, define_text_body,
    font3_body,
};

const GREEN: [u8; 4] = [0, 255, 0, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const FIELD: [i32; 4] = [0, 2000, 800, 1400];
const HEIGHT: u16 = 400;
const FONT: u16 = 3;

fn square(code: char, advance: i16) -> TestGlyph {
    TestGlyph {
        code: u16::try_from(u32::from(code)).unwrap_or(0),
        advance,
        records: vec![
            TestShapeRecord::Move {
                to: (0, 0),
                fill0: 0,
                fill1: 1,
                line: 0,
            },
            TestShapeRecord::Line(10_000, 0),
            TestShapeRecord::Line(0, -14_000),
            TestShapeRecord::Line(-10_000, 0),
            TestShapeRecord::Line(0, 14_000),
        ],
    }
}

fn with_font() -> p4kx_testkit::TestMovie {
    stage().tag(
        75,
        &font3_body(
            FONT,
            "Orbitron",
            &[square('A', 12_000), square('C', 12_000)],
        ),
    )
}

pub(crate) fn field(text: &TestEditText) -> Vec<u8> {
    with_font()
        .tag(37, &text.body())
        .tag(26, &TestPlace::new(1, 7).body2())
        .tag(1, &[])
        .bytes()
}

fn compose(bytes: &[u8], options: &TextOptions<'_>) -> Scene {
    let movie = Movie::parse(bytes.to_vec()).unwrap_or_else(|error| unreachable!("{error}"));
    let assets = movie.assets();
    assert!(assets.failures.is_empty(), "{:?}", assets.failures);
    movie.first_frame_with_text(&assets, options)
}

fn rasterize(scene: &Scene) -> Raster {
    let options = RasterOptions {
        size: CanvasSize::Stage,
        background: Background::Transparent,
    };
    scene
        .rasterize(&HashMap::new(), &options)
        .unwrap_or_else(|error| unreachable!("{error}"))
}

fn render(bytes: &[u8]) -> Raster {
    rasterize(&compose(bytes, &TextOptions::default()))
}

fn codes(raster: &Raster) -> Vec<&'static str> {
    raster
        .skipped
        .iter()
        .map(|skipped| skipped.reason.code())
        .collect()
}

fn drawn(image: &RgbaImage, x: std::ops::Range<u32>, y: std::ops::Range<u32>) -> usize {
    y.flat_map(|y| x.clone().map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(image, x, y) != CLEAR)
        .count()
}

pub(crate) fn plain(text: &str) -> TestEditText {
    TestEditText::new(7, FIELD)
        .text(text)
        .font(FONT, HEIGHT)
        .color(GREEN)
}

#[test]
fn a_static_text_draws_its_glyphs_with_size_colour_and_position() {
    let records = [TestTextRecord {
        font: Some((FONT, HEIGHT)),
        color: Some(RED),
        x: Some(200),
        y: Some(1000),
        glyphs: vec![(0, 240), (0, 240)],
    }];
    let bytes = with_font()
        .tag(
            11,
            &define_text_body(7, [0, 2000, 0, 2000], false, &records),
        )
        .tag(26, &TestPlace::new(1, 7).body2())
        .tag(1, &[])
        .bytes();
    let raster = render(&bytes);
    assert!(raster.skipped.is_empty(), "{:?}", raster.skipped);
    assert_pixel(&raster.image, (15, 43), RED, 0);
    assert_pixel(&raster.image, (27, 43), RED, 0);
    assert_pixel(&raster.image, (20, 43), CLEAR, 0);
    assert_pixel(&raster.image, (15, 30), CLEAR, 0);
    assert_pixel(&raster.image, (15, 52), CLEAR, 0);
}

#[test]
fn a_static_text_with_alpha_and_a_later_record_keeps_the_state() {
    let records = [
        TestTextRecord {
            font: Some((FONT, HEIGHT)),
            color: Some([0, 0, 255, 255]),
            x: Some(200),
            y: Some(1000),
            glyphs: vec![(0, 240)],
        },
        TestTextRecord {
            glyphs: vec![(1, 240)],
            ..TestTextRecord::default()
        },
    ];
    let bytes = with_font()
        .tag(33, &define_text_body(7, [0, 2000, 0, 2000], true, &records))
        .tag(26, &TestPlace::new(1, 7).body2())
        .tag(1, &[])
        .bytes();
    let raster = render(&bytes);
    assert_pixel(&raster.image, (15, 43), BLUE, 0);
    assert_pixel(&raster.image, (27, 43), BLUE, 0);
}

#[test]
fn a_text_field_draws_its_text_from_the_left_of_its_bounds() {
    let raster = render(&field(&plain("AA")));
    assert!(raster.skipped.is_empty(), "{:?}", raster.skipped);
    assert_pixel(&raster.image, (6, 36), GREEN, 0);
    assert_pixel(&raster.image, (18, 36), GREEN, 0);
    assert_pixel(&raster.image, (12, 36), CLEAR, 0);
    assert_pixel(&raster.image, (6, 20), CLEAR, 0);
}

#[test]
fn a_text_field_aligns_right_within_its_bounds() {
    let aligned = plain("AA").layout(1, (0, 0), 0, 0);
    let raster = render(&field(&aligned));
    assert_pixel(&raster.image, (79, 36), GREEN, 0);
    assert_pixel(&raster.image, (6, 36), CLEAR, 0);
    let centred = plain("AA").layout(2, (0, 0), 0, 0);
    let raster = render(&field(&centred));
    assert_pixel(&raster.image, (46, 36), GREEN, 0);
    assert_pixel(&raster.image, (6, 36), CLEAR, 0);
}

#[test]
fn html_text_takes_colour_and_size_from_its_markup() {
    let markup = plain("<p><font color=\"#0000FF\">A</font>&#67;</p>").html();
    let raster = render(&field(&markup));
    assert_pixel(&raster.image, (6, 36), BLUE, 0);
    assert_pixel(&raster.image, (18, 36), GREEN, 0);
    let small = plain("<font size=\"10\">A</font>").html();
    let raster = render(&field(&small));
    assert_pixel(&raster.image, (4, 37), GREEN, 0);
    assert_pixel(&raster.image, (6, 33), CLEAR, 0);
}

#[test]
fn a_password_field_shows_asterisks_not_its_text() {
    let raster = render(&field(&plain("AA").password()));
    assert_eq!(
        codes(&raster),
        ["missing_glyph"],
        "the font has no asterisk"
    );
    assert!(drawn(&raster.image, 0..14, 29..43) > 0, "boxes");
    assert_pixel(&raster.image, (6, 36), CLEAR, 0);
}

#[test]
fn missing_fonts_and_glyphs_are_drawn_as_boxes_and_listed() {
    let raster = render(&field(&plain("AB")));
    assert_eq!(codes(&raster), ["missing_glyph"]);
    assert_eq!(
        raster.skipped[0].reason,
        SkipReason::MissingGlyph { code: 66 }
    );
    assert_pixel(&raster.image, (6, 36), GREEN, 0);
    assert!(drawn(&raster.image, 15..25, 29..43) > 10, "a box for B");
    let no_font = TestEditText::new(7, FIELD).text("AB").color(GREEN);
    let raster = render(&field(&no_font));
    assert_eq!(codes(&raster), ["missing_font"]);
    assert!(drawn(&raster.image, 0..50, 0..50) > 10, "boxes");
    let unknown = TestEditText::new(7, FIELD)
        .text("A")
        .font_class("Nowhere", HEIGHT);
    let raster = render(&field(&unknown));
    assert_eq!(codes(&raster), ["missing_font"]);
}

#[test]
fn a_font_class_names_the_font_and_a_static_text_with_a_missing_font_is_listed() {
    let by_name = TestEditText::new(7, FIELD)
        .text("A")
        .font_class("orbitron", HEIGHT)
        .color(GREEN);
    let raster = render(&field(&by_name));
    assert!(raster.skipped.is_empty(), "{:?}", raster.skipped);
    assert_pixel(&raster.image, (6, 36), GREEN, 0);
    let records = [TestTextRecord {
        font: Some((99, HEIGHT)),
        color: Some(RED),
        x: Some(200),
        y: Some(1000),
        glyphs: vec![(0, 240), (9, 240)],
    }];
    let bytes = with_font()
        .tag(
            11,
            &define_text_body(7, [0, 2000, 0, 2000], false, &records),
        )
        .tag(26, &TestPlace::new(1, 7).body2())
        .tag(1, &[])
        .bytes();
    let raster = render(&bytes);
    assert_eq!(codes(&raster), ["missing_font"]);
    assert!(drawn(&raster.image, 5..40, 30..52) > 10, "boxes");
    let records = [TestTextRecord {
        font: Some((FONT, HEIGHT)),
        color: Some(RED),
        x: Some(200),
        y: Some(1000),
        glyphs: vec![(0, 240), (9, 240)],
    }];
    let bytes = with_font()
        .tag(
            11,
            &define_text_body(7, [0, 2000, 0, 2000], false, &records),
        )
        .tag(26, &TestPlace::new(1, 7).body2())
        .tag(1, &[])
        .bytes();
    let raster = render(&bytes);
    assert_eq!(codes(&raster), ["missing_glyph_index"]);
}

#[test]
fn keys_are_localised_with_their_parameters_and_unknown_keys_are_listed() {
    let lookup = |key: &str| match key {
        "ui_letter" => Some("A".to_owned()),
        "ui_format" => Some("%ls".to_owned()),
        _ => None,
    };
    let options = TextOptions {
        fonts: &[],
        localise: Some(&lookup),
    };
    for text in ["@ui_letter", "@ui_format,P=A", "<font>@ui_letter</font>"] {
        let bytes = field(&plain(text).html());
        let raster = rasterize(&compose(&bytes, &options));
        assert!(raster.skipped.is_empty(), "{text}: {:?}", raster.skipped);
        assert_pixel(&raster.image, (6, 36), GREEN, 0);
    }
    let bytes = field(&plain("@ui_unknown"));
    let raster = rasterize(&compose(&bytes, &options));
    assert_eq!(codes(&raster)[0], "unlocalised");
    let without = rasterize(&compose(
        &field(&plain("@ui_letter")),
        &TextOptions::default(),
    ));
    assert!(
        codes(&without).iter().all(|code| *code != "unlocalised"),
        "without a lookup the key is plain text"
    );
}

#[test]
fn a_localised_text_beyond_the_character_limit_is_cut_and_listed() {
    let lookup = |key: &str| (key == "ui_long").then(|| "A".repeat(5000));
    let options = TextOptions {
        fonts: &[],
        localise: Some(&lookup),
    };
    let raster = rasterize(&compose(&field(&plain("@ui_long")), &options));
    assert_eq!(codes(&raster), ["text_truncated"]);
}

#[test]
fn fonts_of_other_movies_are_found_by_name_only() {
    let donor = Movie::parse(with_font().tag(1, &[]).bytes())
        .unwrap_or_else(|error| unreachable!("{error}"));
    let donor_assets = donor.assets();
    let options = TextOptions {
        fonts: &donor_assets.fonts,
        localise: None,
    };
    let named = stage()
        .tag(
            37,
            &TestEditText::new(7, FIELD)
                .text("A")
                .font_class("Orbitron", HEIGHT)
                .color(GREEN)
                .body(),
        )
        .tag(26, &TestPlace::new(1, 7).body2())
        .tag(1, &[])
        .bytes();
    let raster = rasterize(&compose(&named, &options));
    assert!(raster.skipped.is_empty(), "{:?}", raster.skipped);
    assert_pixel(&raster.image, (6, 36), GREEN, 0);

    let by_id = stage()
        .tag(37, &plain("A").body())
        .tag(26, &TestPlace::new(1, 7).body2())
        .tag(1, &[])
        .bytes();
    let raster = rasterize(&compose(&by_id, &options));
    assert!(
        codes(&raster).contains(&"missing_font"),
        "{:?}",
        raster.skipped
    );
}

#[test]
fn the_svg_writes_text_as_paths_and_skips_what_the_raster_skips() {
    let bytes = field(&plain("A<&B\"").html().multiline());
    let scene = compose(&bytes, &TextOptions::default());
    let raster = rasterize(&scene);
    let options = RasterOptions {
        size: CanvasSize::Stage,
        background: Background::Transparent,
    };
    let mut out = Vec::new();
    let report = scene
        .write_svg(&HashMap::new(), &options, Some("t"), &mut out)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let svg = String::from_utf8(out).unwrap_or_default();
    assert!(!svg.contains("<text") && !svg.contains("<tspan"), "{svg}");
    assert!(
        !svg.contains("A&lt;") && !svg.contains("A<&"),
        "no text content"
    );
    assert!(svg.contains("<path"), "{svg}");
    assert!(
        svg.contains("fill=\"#00ff00\""),
        "the colour of the text: {svg}"
    );
    assert_eq!(
        report.skipped, raster.skipped,
        "the same elements are skipped"
    );
    assert!(
        raster
            .skipped
            .iter()
            .any(|skipped| skipped.reason.code() == "missing_glyph"),
        "<, &, B and the quote are not in the font"
    );
}

#[test]
fn damaged_text_tags_are_reported_and_the_rest_still_renders() {
    let bytes = with_font()
        .tag(37, &[7, 0])
        .tag(11, &[8, 0, 1])
        .tag(1, &[])
        .bytes();
    let movie = Movie::parse(bytes).unwrap_or_else(|error| unreachable!("{error}"));
    let assets = movie.assets();
    assert_eq!(assets.failures.len(), 2, "{:?}", assets.failures);
    assert!(assets.edit_texts.is_empty() && assets.static_texts.is_empty());
    let scene = movie.first_frame(&assets);
    assert_eq!(scene.items.len(), 0);
}
