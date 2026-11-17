// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{LayerPlan, Layered, MAX_BAKED_LAYERS};

fn layer(at: usize, share: f32) -> LayerPlan {
    LayerPlan {
        gloss: Vec::new(),
        finish: None,
        roughness: 0.5,
        at,
        share,
        diffuse: None,
        normal: None,
        colour: [1.0; 3],
        specular: [0.0; 3],
        metal: 0.0,
    }
}

fn shares_at(layered: &Layered, mask: Option<[f32; 3]>) -> [f32; MAX_BAKED_LAYERS] {
    let mut shares = [0.0; MAX_BAKED_LAYERS];
    layered.shares(mask, &mut shares);
    shares
}

fn close(found: [f32; MAX_BAKED_LAYERS], wanted: &[f32]) -> bool {
    found
        .iter()
        .zip(wanted.iter().chain(std::iter::repeat(&0.0)))
        .all(|(found, wanted)| (found - wanted).abs() < 1e-6)
}

#[test]
fn the_mask_channels_choose_the_base_layers_and_the_wear_share_stays() {
    let layered = Layered {
        layers: (0..5)
            .map(|at| layer(at, if at == 4 { 0.2 } else { 0.0 }))
            .collect(),
        mask: None,
        refused: Vec::new(),
    };

    assert!(close(
        shares_at(&layered, Some([0.5, 0.0, 0.0])),
        &[0.4, 0.0, 0.0, 0.4, 0.2]
    ));
    assert!(close(
        shares_at(&layered, Some([0.0, 0.0, 1.0])),
        &[0.0, 0.0, 0.8, 0.0, 0.2]
    ));
    assert!(close(
        shares_at(&layered, Some([0.0; 3])),
        &[0.0, 0.0, 0.0, 0.8, 0.2]
    ));
}

#[test]
fn red_goes_over_green_and_green_over_blue() {
    let layered = Layered {
        layers: (0..4).map(|at| layer(at, 0.0)).collect(),
        mask: None,
        refused: Vec::new(),
    };

    assert!(close(
        shares_at(&layered, Some([1.0, 0.0, 1.0])),
        &[1.0, 0.0, 0.0, 0.0]
    ));
    assert!(close(
        shares_at(&layered, Some([0.0, 1.0, 1.0])),
        &[0.0, 1.0, 0.0, 0.0]
    ));
    assert!(close(
        shares_at(&layered, Some([1.0, 1.0, 0.0])),
        &[1.0, 0.0, 0.0, 0.0]
    ));
}

#[test]
fn without_a_mask_the_shares_are_those_by_position() {
    let layered = Layered {
        layers: vec![layer(0, 0.75), layer(1, 0.25)],
        mask: None,
        refused: Vec::new(),
    };

    assert!(close(shares_at(&layered, None), &[0.75, 0.25]));
}

#[test]
fn without_a_fourth_layer_the_last_base_layer_present_takes_what_no_channel_claims() {
    let layered = Layered {
        layers: (0..3).map(|at| layer(at, 0.0)).collect(),
        mask: None,
        refused: Vec::new(),
    };

    assert!(close(shares_at(&layered, Some([0.0; 3])), &[0.0, 0.0, 1.0]));
    assert!(close(
        shares_at(&layered, Some([0.0, 1.0, 0.0])),
        &[0.0, 1.0, 0.0]
    ));
    assert!(close(
        shares_at(&layered, Some([0.5, 0.0, 0.0])),
        &[0.5, 0.0, 0.5]
    ));
}

#[test]
fn a_missing_base_layer_is_skipped_and_the_channels_stay_with_the_positions() {
    let layered = Layered {
        layers: [0, 2, 3, 4]
            .map(|at| layer(at, if at == 4 { 0.2 } else { 0.0 }))
            .to_vec(),
        mask: None,
        refused: Vec::new(),
    };

    assert!(close(
        shares_at(&layered, Some([0.0, 1.0, 0.0])),
        &[0.0, 0.0, 0.8, 0.2]
    ));
    assert!(close(
        shares_at(&layered, Some([0.0, 0.0, 1.0])),
        &[0.0, 0.8, 0.0, 0.2]
    ));
    assert!(close(
        shares_at(&layered, Some([1.0, 1.0, 1.0])),
        &[0.8, 0.0, 0.0, 0.2]
    ));
}

#[test]
fn a_mask_with_no_base_layer_present_gives_the_shares_by_position() {
    let layered = Layered {
        layers: vec![layer(4, 0.7), layer(5, 0.3)],
        mask: None,
        refused: Vec::new(),
    };

    assert!(close(
        shares_at(&layered, Some([0.0, 0.0, 1.0])),
        &[0.7, 0.3]
    ));
}
