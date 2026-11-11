// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The display list of the first frame: placements, changes and removals with matrices, colour
//! transforms and the `PlaceObject3` extras.

use p4kx_gfx::{
    ColorTransform, FilterKind, GfxError, MAX_FRAME_PLACEMENTS, Matrix, Movie, OtherDefinition,
    Rgba,
};
use p4kx_testkit::{
    TestMovie, TestPlace, TestSignature, background_body, remove_object2_body, sprite_body,
};

fn movie(bytes: &[u8]) -> Movie {
    Movie::parse(bytes.to_vec()).unwrap_or_else(|error| unreachable!("invalid movie: {error}"))
}

fn placements() -> Vec<u8> {
    TestMovie::new(TestSignature::Gfx)
        .tag(9, &background_body([10, 20, 30]))
        .tag(34, &[42, 0, 0])
        .tag(26, &TestPlace::new(1, 5).at(100, -40).body2())
        .tag(
            70,
            &TestPlace::new(2, 6)
                .matrix([2.0, 0.5, 0.25, -0.25], (2000, 0))
                .color([0.5, 1.0, 1.0, 0.25], [10, -20, 0, 0])
                .name("needle")
                .blur()
                .blend_mode(3)
                .visible(false)
                .body3(),
        )
        .tag(26, &TestPlace::new(3, 7).clip(9).ratio(100).body2())
        .tag(26, &TestPlace::modify(1).at(0, 60).body2())
        .tag(28, &remove_object2_body(3))
        .tag(1, &[])
        .tag(26, &TestPlace::new(4, 8).body2())
        .bytes()
}

#[test]
fn the_first_frame_keeps_what_its_tags_place_change_and_remove() {
    let movie = movie(&placements());
    let assets = movie.assets();
    assert!(assets.failures.is_empty(), "{:?}", assets.failures);
    assert_eq!(
        assets.background,
        Some(Rgba {
            red: 10,
            green: 20,
            blue: 30,
            alpha: 255
        }),
        "background"
    );
    assert_eq!(
        assets.other_definitions,
        [OtherDefinition { id: 42, code: 34 }],
        "a button"
    );
    let frame = &assets.timeline.first_frame;
    assert_eq!(
        frame
            .iter()
            .map(|placed| (placed.depth, placed.id))
            .collect::<Vec<_>>(),
        [(1, 5), (2, 6)],
        "depth 3 removed, depth 4 placed in the second frame"
    );
    assert_eq!(
        frame[0].matrix,
        Matrix {
            translate_x: 0,
            translate_y: 60,
            ..Matrix::IDENTITY
        },
        "moved"
    );
    let needle = &frame[1];
    assert_eq!(
        (needle.matrix.scale_x, needle.matrix.scale_y),
        (2.0, 0.5),
        "scale"
    );
    assert_eq!(
        (needle.matrix.rotate_skew0, needle.matrix.rotate_skew1),
        (0.25, -0.25),
        "skew"
    );
    assert_eq!(
        needle.color_transform,
        ColorTransform {
            multiply: [0.5, 1.0, 1.0, 0.25],
            add: [10.0, -20.0, 0.0, 0.0],
        },
        "colour transform"
    );
    assert_eq!(needle.name.as_deref(), Some("needle"), "name");
    assert_eq!(needle.blend_mode, 3, "blend mode after the filters");
    assert!(!needle.visible, "hidden");
    assert_eq!(assets.timeline.placed, [5, 6, 7, 8], "every placed ID");
}

#[test]
fn the_filters_of_a_placement_are_kept_by_kind_with_their_parameters() {
    let movie = movie(&placements());
    let assets = movie.assets();
    let filters: Vec<(FilterKind, usize)> = assets.timeline.first_frame[1]
        .filters
        .iter()
        .map(|filter| (filter.kind, filter.data.len()))
        .collect();
    assert_eq!(filters, [(FilterKind::Blur, 9)], "filters");
}

#[test]
fn a_sprite_has_its_own_first_frame() {
    let bytes = TestMovie::new(TestSignature::Gfx)
        .tag(
            39,
            &sprite_body(
                9,
                2,
                &[
                    (26, TestPlace::new(1, 4).clip(3).body2()),
                    (1, Vec::new()),
                    (28, remove_object2_body(1)),
                    (1, Vec::new()),
                ],
            ),
        )
        .bytes();
    let movie = movie(&bytes);
    let assets = movie.assets();
    let frame = &assets.sprites[0].timeline.first_frame;
    assert_eq!(frame.len(), 1, "the removal comes in the second frame");
    assert_eq!(frame[0].clip_depth, Some(3), "a mask");
    assert!(assets.timeline.first_frame.is_empty(), "nothing placed");
}

#[test]
fn colour_transforms_compose_and_apply_with_clamping() {
    let outer = ColorTransform {
        multiply: [0.5, 1.0, 1.0, 1.0],
        add: [0.0, 0.0, 100.0, 0.0],
    };
    let inner = ColorTransform {
        multiply: [1.0, 2.0, 1.0, 0.5],
        add: [20.0, 0.0, 0.0, 0.0],
    };
    let color = Rgba {
        red: 100,
        green: 200,
        blue: 200,
        alpha: 200,
    };
    assert_eq!(
        outer.compose(&inner).apply(color),
        Rgba {
            red: 60,
            green: 255,
            blue: 255,
            alpha: 100
        },
        "(100 + 20) / 2, 400 clamped, 300 clamped, 200 / 2"
    );
    assert!(ColorTransform::IDENTITY.is_identity(), "identity");
}

#[test]
fn long_names_over_many_frames_end_in_a_typed_error_and_a_small_result() {
    let name = "n".repeat(60_000);
    let mut builder =
        TestMovie::new(TestSignature::Gfx).tag(26, &TestPlace::new(1, 5).name(&name).body2());
    for _ in 0..5_000 {
        builder = builder.tag(1, &[]);
    }
    let movie = movie(&builder.bytes());
    let assets = movie.assets();
    let kept: usize = assets.timeline.frame_lists.iter().map(Vec::len).sum();
    assert!(kept < 2_000, "the byte budget stops the copies: {kept}");
    assert!(
        assets
            .failures
            .iter()
            .any(|failure| matches!(failure.error, GfxError::FrameListsTooLarge { .. })),
        "{:?}",
        assets.failures
    );
    assert_eq!(
        assets.timeline.frames, 5_000,
        "all frames are still counted"
    );
}

#[test]
fn frame_lists_stop_at_the_placement_cap_with_one_typed_failure() {
    const SHOW_FRAME: [u8; 2] = [0x40, 0x00];
    let frames = MAX_FRAME_PLACEMENTS + 1_000;
    let builder = TestMovie::new(TestSignature::Gfx).raw(&SHOW_FRAME.repeat(frames));
    let movie = movie(&builder.bytes());
    let assets = movie.assets();
    assert_eq!(
        assets.timeline.frame_lists.len(),
        MAX_FRAME_PLACEMENTS,
        "a frame counts as one placement, and no frame beyond the cap is kept"
    );
    let capped: Vec<_> = assets
        .failures
        .iter()
        .filter(|failure| matches!(failure.error, GfxError::TooManyFramePlacements { .. }))
        .collect();
    assert_eq!(capped.len(), 1, "{:?}", assets.failures);
    assert!(matches!(
        capped[0].error,
        GfxError::TooManyFramePlacements {
            limit: MAX_FRAME_PLACEMENTS
        }
    ));
    assert!(
        !assets
            .failures
            .iter()
            .any(|failure| matches!(failure.error, GfxError::FrameListsTooLarge { .. })),
        "{:?}",
        assets.failures
    );
    assert_eq!(
        usize::try_from(assets.timeline.frames),
        Ok(frames),
        "all frames are still counted"
    );
}

#[test]
fn first_frame_receipts_follow_effective_placement_properties_and_removals() {
    let input = placements();
    let parsed = movie(&input);
    let observed = parsed.assets_observed();
    let frame = &observed.origins.timeline.first_frame;
    let depths: Vec<_> = frame.keys().copied().collect();
    assert_eq!(depths, [1, 2], "removed and later placements are excluded");

    let edited = &frame[&1];
    let original = edited.character.expect("initial character");
    let transform = edited.matrix.expect("last authored matrix");
    assert_eq!(
        input.get(original.body.offset..original.body.end()),
        Some(TestPlace::new(1, 5).at(100, -40).body2().as_slice()),
        "the initial tag owns the effective character selection"
    );
    assert_eq!(
        input.get(transform.body.offset..transform.body.end()),
        Some(TestPlace::modify(1).at(0, 60).body2().as_slice()),
        "the move-only tag owns the effective transform"
    );
    assert_eq!(
        frame[&2].filters, frame[&2].visible,
        "the same PlaceObject3 tag supplied both authored effects"
    );
}

#[test]
fn replacing_a_character_without_move_drops_the_prior_property_origins() {
    let new_character = TestPlace::new(1, 7).body2();
    let replaced = TestMovie::new(TestSignature::Gfx)
        .tag(26, &TestPlace::new(1, 5).at(100, -40).body2())
        .tag(26, &new_character)
        .bytes();
    let parsed = movie(&replaced);
    let observed = parsed.assets_observed();
    let origin = &observed.origins.timeline.first_frame[&1];
    let selected = origin.character.expect("replacement character");
    assert_eq!(
        replaced.get(selected.body.offset..selected.body.end()),
        Some(new_character.as_slice()),
        "the replacement tag selected the character"
    );
    assert_eq!(
        origin.matrix, None,
        "identity is implicit after a complete replacement"
    );
}
