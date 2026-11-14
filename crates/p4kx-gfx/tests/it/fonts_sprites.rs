// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Fonts with their glyphs, codes and layout, and sprites with their nested timelines.

use p4kx_gfx::{FontFormat, Movie, Point, ShapeRecord};
use p4kx_testkit::{
    TestGlyph, TestMovie, TestShape, TestShapeRecord, TestSignature, font_name_body, font3_body,
    place_object2_body, sprite_body,
};

fn glyph(code: u16, advance: i16, width: i32) -> TestGlyph {
    TestGlyph {
        code,
        advance,
        records: vec![
            TestShapeRecord::Move {
                to: (0, 0),
                fill0: 1,
                fill1: 0,
                line: 0,
            },
            TestShapeRecord::Line(width, 0),
            TestShapeRecord::Line(0, -900),
        ],
    }
}

fn parse(movie: &TestMovie) -> Movie {
    Movie::parse(movie.bytes()).unwrap_or_else(|error| unreachable!("invalid movie: {error}"))
}

#[test]
fn a_define_font3_reads_glyphs_codes_layout_and_full_name() {
    let glyphs = [
        glyph(u16::from(b'A'), 1100, 1000),
        glyph(0x00C4, 1200, 1100),
    ];
    let movie = parse(
        &TestMovie::new(TestSignature::Gfx)
            .tag(75, &font3_body(3, "Electrolize", &glyphs))
            .tag(88, &font_name_body(3, "Electrolize Regular")),
    );
    let assets = movie.assets();
    assert!(assets.failures.is_empty(), "{:?}", assets.failures);
    let font = &assets.fonts[0];
    assert_eq!(font.id, 3, "id");
    assert_eq!(font.format, FontFormat::DefineFont3, "format");
    assert_eq!(font.name, "Electrolize", "name");
    assert_eq!(
        font.full_name.as_deref(),
        Some("Electrolize Regular"),
        "full name"
    );
    assert!(font.bold && !font.italic, "bold");
    assert_eq!(font.glyph_count, 2, "glyph count");
    let codes: Vec<Option<u16>> = font.glyphs.iter().map(|glyph| glyph.code).collect();
    assert_eq!(codes, [Some(65), Some(0xC4)], "codes");
    assert_eq!(font.glyphs[1].advance, Some(1200), "advance");
    assert_eq!(
        font.glyphs[1].bounds.map(|bounds| bounds.y_min),
        Some(-900),
        "bounds"
    );
    assert_eq!(
        font.glyphs[1].records[1],
        ShapeRecord::StraightEdge {
            delta: Point { x: 1100, y: 0 }
        },
        "outline"
    );
    let layout = font.layout.as_ref().expect("layout");
    assert_eq!((layout.ascent, layout.descent), (900, 200), "metrics");
    assert_eq!(layout.kerning.len(), 1, "one pair");
    assert_eq!(layout.kerning[0].adjustment, -40, "kerning");
}

#[test]
fn a_font_whose_glyph_shapes_were_stripped_keeps_its_codes_and_layout() {
    let mut body = 1_u16.to_le_bytes().to_vec();
    body.extend_from_slice(&[0x84, 1, 9]);
    body.extend_from_slice(b"Orbitron\0");
    body.extend_from_slice(&2_u16.to_le_bytes());
    for value in [0_u16, 4, 0x20, 0x41, 900, 200, 0, 500, 600] {
        body.extend_from_slice(&value.to_le_bytes());
    }
    body.extend_from_slice(&[0x08, 0x00, 0x08, 0x00]);
    body.extend_from_slice(&0_u16.to_le_bytes());
    let movie = parse(&TestMovie::new(TestSignature::Gfx).tag(75, &body));
    let assets = movie.assets();
    assert!(assets.failures.is_empty(), "{:?}", assets.failures);
    let font = &assets.fonts[0];
    assert_eq!(font.name, "Orbitron", "name without the NUL");
    assert_eq!(font.glyph_count, 2, "glyph count");
    let codes: Vec<Option<u16>> = font.glyphs.iter().map(|glyph| glyph.code).collect();
    assert_eq!(codes, [Some(0x20), Some(0x41)], "codes");
    assert!(
        font.glyphs.iter().all(|glyph| glyph.records.is_empty()),
        "no outlines"
    );
    assert_eq!(font.glyphs[1].advance, Some(600), "advance");
}

#[test]
fn a_define_font4_reads_its_name_and_style() {
    let mut body = 12_u16.to_le_bytes().to_vec();
    body.push(0x06);
    body.extend_from_slice(b"Lato\0CFF");
    let movie = parse(&TestMovie::new(TestSignature::Fws).tag(91, &body));
    let assets = movie.assets();
    let font = &assets.fonts[0];
    assert_eq!(font.format, FontFormat::DefineFont4, "format");
    assert_eq!((font.id, font.name.as_str()), (12, "Lato"), "id and name");
    assert!(font.italic && !font.bold, "italic");
}

#[test]
fn a_compacted_font_reads_its_name_and_metrics() {
    let mut body = 9_u16.to_le_bytes().to_vec();
    body.extend_from_slice(b"Orbitron\0");
    for value in [0_u16, 1024, 800, 224, 0] {
        body.extend_from_slice(&value.to_le_bytes());
    }
    body.extend_from_slice(&95_u32.to_le_bytes());
    let movie = parse(&TestMovie::new(TestSignature::Gfx).tag(1005, &body));
    let assets = movie.assets();
    let font = &assets.fonts[0];
    assert_eq!(font.format, FontFormat::Compacted, "format");
    assert_eq!(font.name, "Orbitron", "name");
    assert_eq!(font.glyph_count, 95, "glyph count");
    assert!(font.glyphs.is_empty(), "glyph shapes not read");
    assert_eq!(
        font.layout.as_ref().map(|layout| layout.ascent),
        Some(800),
        "ascent"
    );
}

#[test]
fn a_sprite_reads_its_timeline_and_its_nested_definitions() {
    let shape = TestShape::new(11, 1, [0, 20, 0, 20]);
    let inner = sprite_body(21, 1, &[(26, place_object2_body(1, 11)), (1, Vec::new())]);
    let outer = sprite_body(
        20,
        2,
        &[
            (shape.code(), shape.body()),
            (39, inner),
            (43, b"open\0".to_vec()),
            (26, place_object2_body(1, 21)),
            (1, Vec::new()),
            (26, place_object2_body(2, 11)),
            (1, Vec::new()),
        ],
    );
    let movie = parse(&TestMovie::new(TestSignature::Fws).tag(39, &outer));
    let assets = movie.assets();
    assert!(assets.failures.is_empty(), "{:?}", assets.failures);
    assert_eq!(
        assets.shapes[0].id, 11,
        "the shape inside the sprite is read"
    );
    let ids: Vec<u16> = assets.sprites.iter().map(|sprite| sprite.id).collect();
    assert_eq!(ids, [20, 21], "parent first");
    let outer = &assets.sprites[0];
    assert_eq!(outer.frame_count, 2, "declared frames");
    assert_eq!(outer.timeline.frames, 2, "frames");
    assert_eq!(outer.timeline.frame_of("OPEN"), Some(0), "labels");
    assert_eq!(outer.timeline.placed, [11, 21], "placed");
    assert_eq!(outer.timeline.tag_count, 8, "tags with End");
    assert_eq!(assets.sprites[1].timeline.placed, [11], "nested");
}

#[test]
fn deeply_nested_sprite_definitions_and_following_tags_are_all_read() {
    let depth = 512_u16;
    let mut body = sprite_body(1, 1, &[(43, b"leaf\0".to_vec()), (1, Vec::new())]);
    for id in 2..=depth {
        body = sprite_body(id, 1, &[(39, body)]);
    }
    let movie = parse(
        &TestMovie::new(TestSignature::Gfx)
            .tag(39, &body)
            .tag(43, b"after\0"),
    );
    let assets = movie.assets();
    assert!(assets.failures.is_empty(), "{:?}", assets.failures);
    assert!(
        assets
            .sprites
            .iter()
            .map(|sprite| sprite.id)
            .eq((1..=depth).rev()),
        "every physical definition is retained in parent-first source order"
    );
    let leaf = assets.sprites.last().expect("nested leaf");
    assert_eq!(leaf.timeline.frame_of("leaf"), Some(0));
    assert_eq!(leaf.timeline.frames, 1);
    assert_eq!(assets.timeline.frame_of("after"), Some(0));
    assert_eq!(
        assets
            .tag_counts
            .iter()
            .find(|tag| tag.code == 39)
            .map(|tag| tag.count),
        Some(u32::from(depth))
    );
}

#[test]
fn shorter_font_info_overrides_only_the_glyph_code_prefix_it_supplies() {
    let glyphs = [
        glyph(u16::from(b'D'), 12_000, 10_000),
        glyph(u16::from(b'E'), 12_000, 10_000),
        glyph(u16::from(b'F'), 12_000, 10_000),
    ];
    let all_codes = b"\x04\0\x01a\0ABC";
    let first_code = b"\x04\0\x01b\0Z";
    let only_style = b"\x04\0\x01c\0";
    let input = TestMovie::new(TestSignature::Gfx)
        .tag(75, &font3_body(4, "Original", &glyphs))
        .tag(13, all_codes)
        .tag(13, first_code)
        .tag(13, only_style)
        .bytes();
    let movie = Movie::parse(input.clone()).expect("synthetic movie");
    let observed = movie.assets_observed();
    let font = observed.assets.fonts.first().expect("one font");
    let codes: Vec<_> = font.glyphs.iter().filter_map(|glyph| glyph.code).collect();
    assert_eq!(codes, [90, 66, 67], "the effective codes use both tags");
    assert_eq!(font.name, "c", "last info supplies the name");

    let origins = observed.origins.fonts.first().expect("font origins");
    let name = origins.info.expect("font name override");
    assert_eq!(
        input.get(name.body.offset..name.body.end()),
        Some(only_style.as_slice()),
        "the latest tag supplied the name independently of the codes"
    );
    for (index, body) in [first_code.as_slice(), all_codes, all_codes]
        .into_iter()
        .enumerate()
    {
        let site = origins.glyph_code_override(index).expect("code override");
        assert_eq!(site.ordinal, index, "index inside the accepted tag");
        assert_eq!(
            input.get(site.tag.body.offset..site.tag.body.end()),
            Some(body),
            "the retained code source for glyph {index}"
        );
    }
}
