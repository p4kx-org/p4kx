// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gfx::{Movie, SceneDefinitionOrigin, TextOptions};
use p4kx_testkit::{TestFill, TestPlace, sprite_body};

use crate::support::{BLUE, RED, rect, stage};
use crate::text::{field, plain};

#[test]
fn composition_keeps_actual_shape_and_sprite_occurrences_through_nested_instances() {
    let selected = rect(4, (1000, 1000), TestFill::Solid(RED));
    let duplicate = rect(4, (2000, 2000), TestFill::Solid(BLUE));
    let child = TestPlace::new(2, 4).at(200, 0).body2();
    let sprite = sprite_body(9, 1, &[(26, child), (1, Vec::new())]);
    let empty_duplicate = sprite_body(9, 1, &[(1, Vec::new())]);
    let bytes = stage()
        .tag(selected.code(), &selected.body())
        .tag(duplicate.code(), &duplicate.body())
        .tag(39, &sprite)
        .tag(39, &empty_duplicate)
        .tag(26, &TestPlace::new(1, 9).body2())
        .tag(26, &TestPlace::new(3, 9).at(800, 0).body2())
        .tag(1, &[])
        .bytes();
    let movie = Movie::parse(bytes).expect("synthetic movie");
    let assets = movie.assets();
    let observed = movie.first_frame_observed(&assets, &TextOptions::default());
    assert_eq!(
        observed.scene,
        movie.first_frame(&assets),
        "the ordinary composition remains identical"
    );
    let placement = |index| observed.origins.placements.get(index).expect("placement");
    let origins: Vec<_> = observed
        .origins
        .items
        .iter()
        .map(|draw| {
            let child = placement(draw.placement.expect("child placement"));
            let parent = placement(child.parent.expect("enclosing sprite placement"));
            (draw.definition, child.timeline, child.depth, parent.depth)
        })
        .collect();
    assert_eq!(
        origins,
        [
            (Some(SceneDefinitionOrigin::Shape(0)), Some(0), 2, 1),
            (Some(SceneDefinitionOrigin::Shape(0)), Some(0), 2, 3)
        ],
        "each instance keeps its distinct enclosing path and the same actual sprite and shape"
    );
}

#[test]
fn the_drawn_glyph_keeps_the_selected_font_and_glyph_ordinals() {
    let bytes = field(&plain("CA"));
    let movie = Movie::parse(bytes).expect("synthetic movie");
    let assets = movie.assets();
    let observed = movie.first_frame_observed(&assets, &TextOptions::default());
    assert_eq!(
        observed.scene,
        movie.first_frame(&assets),
        "the observed composition keeps the same glyphs and placement"
    );
    let glyphs: Vec<_> = observed
        .origins
        .items
        .iter()
        .map(|item| {
            let glyph = observed
                .origins
                .glyphs
                .get(&item.shape)
                .expect("glyph shape");
            (item.definition, glyph.font, glyph.glyph)
        })
        .collect();
    assert_eq!(
        glyphs,
        [
            (Some(SceneDefinitionOrigin::EditText(0)), 0, 1),
            (Some(SceneDefinitionOrigin::EditText(0)), 0, 0)
        ],
        "draws retain the actual code lookup order and their text definition"
    );
}
