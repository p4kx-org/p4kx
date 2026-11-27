// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Imported assets, symbol classes, scaling grids, frame labels, the display list of every frame
//! and fonts imported from another movie.

use std::collections::HashMap;

use crate::support::{assert_pixel, stage};
use p4kx_gfx::{
    Background, CanvasSize, GfxError, MAX_IMPORTS, Movie, Raster, RasterOptions, Rect, TextOptions,
};
use p4kx_testkit::{
    TestEditText, TestGlyph, TestMovie, TestPlace, TestShapeRecord, exports_body, font3_body,
    import_assets_body, remove_object2_body, scaling_grid_body, symbol_class_body,
};

const GREEN: [u8; 4] = [0, 255, 0, 255];
const FIELD: [i32; 4] = [0, 2000, 800, 1400];
const HEIGHT: u16 = 400;

fn parse(movie: &TestMovie) -> Movie {
    Movie::parse(movie.bytes()).unwrap_or_else(|error| unreachable!("invalid movie: {error}"))
}

fn exporter(export: &str) -> Movie {
    let square = TestGlyph {
        code: u16::from(b'A'),
        advance: 12_000,
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
    };
    parse(
        &stage()
            .tag(75, &font3_body(1, "Orbitron", &[square]))
            .tag(56, &exports_body(&[(1, export)])),
    )
}

fn importer(name: &str) -> Movie {
    let text = TestEditText::new(7, FIELD)
        .text("A")
        .font(5, HEIGHT)
        .color(GREEN);
    parse(
        &stage()
            .tag(71, &import_assets_body("fonts.gfx", 2, &[(5, name)]))
            .tag(37, &text.body())
            .tag(26, &TestPlace::new(1, 7).body2())
            .tag(1, &[]),
    )
}

fn render(movie: &Movie, options: &TextOptions<'_>) -> Raster {
    let assets = movie.assets();
    assert!(assets.failures.is_empty(), "{:?}", assets.failures);
    let scene = movie.first_frame_with_text(&assets, options);
    let options = RasterOptions {
        size: CanvasSize::Stage,
        background: Background::Transparent,
    };
    scene
        .rasterize(&HashMap::new(), &options)
        .unwrap_or_else(|error| unreachable!("{error}"))
}

#[test]
fn imports_symbol_classes_and_scaling_grids_are_read() {
    let movie = parse(
        &stage()
            .tag(
                57,
                &import_assets_body("a.gfx", 1, &[(1, "One"), (2, "Two")]),
            )
            .tag(71, &import_assets_body("..\\b.gfx", 2, &[(3, "Three")]))
            .tag(76, &symbol_class_body(&[(0, "Main"), (3, "Panel")]))
            .tag(78, &scaling_grid_body(9, [10, 90, 20, 80])),
    );
    let assets = movie.assets();
    assert!(assets.failures.is_empty(), "{:?}", assets.failures);
    let imports: Vec<(&str, Vec<(u16, &str)>)> = assets
        .imports
        .iter()
        .map(|import| {
            (
                import.url.as_str(),
                import
                    .assets
                    .iter()
                    .map(|asset| (asset.id, asset.name.as_str()))
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        imports,
        [
            ("a.gfx", vec![(1, "One"), (2, "Two")]),
            ("..\\b.gfx", vec![(3, "Three")]),
        ],
        "both tag versions"
    );
    assert_eq!(assets.import_of(3), Some(("Three", "..\\b.gfx")), "by ID");
    assert_eq!(assets.import_of(4), None, "unknown ID");
    let symbols: Vec<(u16, &str)> = assets
        .symbol_classes
        .iter()
        .map(|symbol| (symbol.id, symbol.name.as_str()))
        .collect();
    assert_eq!(symbols, [(0, "Main"), (3, "Panel")], "symbol classes");
    assert_eq!(assets.scaling_grids.len(), 1, "one grid");
    assert_eq!(assets.scaling_grids[0].id, 9, "grid ID");
    assert_eq!(
        assets.scaling_grids[0].splitter,
        Rect {
            x_min: 10,
            x_max: 90,
            y_min: 20,
            y_max: 80
        },
        "splitter"
    );
    assert!(assets.unknown_tags.is_empty(), "known tags");
}

#[test]
fn a_truncated_or_oversized_import_is_a_failure() {
    let mut truncated = b"x.gfx\0".to_vec();
    truncated.extend_from_slice(&u16::MAX.to_le_bytes());
    truncated.extend_from_slice(&[1, 0, b'a', 0]);
    let big: Vec<(u16, &str)> = (0..40_000_u16).map(|id| (id, "n")).collect();
    let movie = parse(
        &stage()
            .tag(57, &truncated)
            .tag(57, &import_assets_body("a.gfx", 1, &big))
            .tag(57, &import_assets_body("b.gfx", 1, &big))
            .tag(1, &[]),
    );
    let assets = movie.assets();
    assert_eq!(assets.timeline.frames, 1, "the rest is read");
    assert_eq!(assets.failures.len(), 2, "{:?}", assets.failures);
    assert!(matches!(assets.failures[0].error, GfxError::Parse(_)));
    assert!(
        matches!(assets.failures[1].error, GfxError::TooManyImports { limit } if limit == MAX_IMPORTS),
        "{:?}",
        assets.failures[1].error
    );
    assert_eq!(assets.imports.len(), 1, "only the first full tag is kept");
}

#[test]
fn frame_labels_carry_their_frame_and_every_frame_has_a_display_list() {
    let movie = parse(
        &stage()
            .tag(43, b"start\0")
            .tag(26, &TestPlace::new(1, 10).body2())
            .tag(1, &[])
            .tag(43, b"Middle\0")
            .tag(26, &TestPlace::new(2, 11).body2())
            .tag(1, &[])
            .tag(28, &remove_object2_body(1))
            .tag(1, &[])
            .tag(43, b"end\0"),
    );
    let assets = movie.assets();
    assert!(assets.failures.is_empty(), "{:?}", assets.failures);
    let timeline = &assets.timeline;
    let labels: Vec<(u32, &str)> = timeline
        .labels
        .iter()
        .map(|label| (label.frame, label.name.as_str()))
        .collect();
    assert_eq!(labels, [(0, "start"), (1, "Middle"), (3, "end")], "labels");
    assert_eq!(timeline.frame_of("middle"), Some(1), "case-insensitive");
    assert_eq!(timeline.frame_of("nowhere"), None, "unknown label");
    let depths = |frame: u32| -> Vec<u16> {
        timeline
            .display_list(frame)
            .unwrap_or_default()
            .iter()
            .map(|placement| placement.depth)
            .collect()
    };
    assert_eq!(depths(0), [1], "frame 0");
    assert_eq!(depths(1), [1, 2], "frame 1");
    assert_eq!(depths(2), [2], "RemoveObject2 applies");
    assert_eq!(timeline.display_list(3), None, "no fourth frame");
    assert_eq!(timeline.first_frame.len(), 1, "the first frame");
}

#[test]
fn a_font_imported_from_another_movie_draws_by_its_export_name() {
    let fonts = exporter("$Orbitron");
    let exported = fonts.assets();
    assert_eq!(
        exported.fonts[0].export_name.as_deref(),
        Some("$Orbitron"),
        "export name"
    );
    let options = TextOptions {
        fonts: &exported.fonts,
        localise: None,
    };
    let raster = render(&importer("$Orbitron"), &options);
    assert!(raster.skipped.is_empty(), "{:?}", raster.skipped);
    assert_pixel(&raster.image, (6, 36), GREEN, 0);
}

#[test]
fn a_font_imported_under_an_unknown_name_is_missing() {
    let fonts = exporter("$Orbitron");
    let exported = fonts.assets();
    let options = TextOptions {
        fonts: &exported.fonts,
        localise: None,
    };
    let raster = render(&importer("$Other"), &options);
    let codes: Vec<&str> = raster
        .skipped
        .iter()
        .map(|skipped| skipped.reason.code())
        .collect();
    assert_eq!(codes, ["missing_font"], "no font of that name");
    let raster = render(&importer("$Orbitron"), &TextOptions::default());
    assert_eq!(raster.skipped.len(), 1, "without the exporting movie");
}

#[test]
fn every_export_item_keeps_the_accepted_occurrence_and_ordinal() {
    let first = exports_body(&[(4, "OtherName"), (4, "Wanted")]);
    let later = exports_body(&[(4, "Wanted")]);
    let input = stage().tag(56, &first).tag(56, &later).bytes();
    let movie = Movie::parse(input.clone()).expect("synthetic movie");
    let observed = movie.assets_observed();
    let items = &observed.origins.export_items;
    let names: Vec<_> = observed
        .assets
        .exports
        .iter()
        .map(|record| record.name.as_str())
        .collect();
    assert_eq!(names, ["OtherName", "Wanted", "Wanted"], "accepted order");
    let original: Vec<_> = items
        .iter()
        .map(|site| {
            (
                site.ordinal,
                input.get(site.tag.body.offset..site.tag.body.end()),
            )
        })
        .collect();
    assert_eq!(
        original,
        [
            (0, Some(first.as_slice())),
            (1, Some(first.as_slice())),
            (0, Some(later.as_slice())),
        ],
        "each alias and repeated name keeps its physical tag and item"
    );
    assert_eq!(
        observed.origins.exports.get(&4),
        items.first(),
        "the existing first-export-per-ID lookup keeps its meaning"
    );
}
