// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{Collected, TextureMode};

#[test]
fn one_path_shares_compatible_output_and_keeps_other_roles_separate() {
    let mut collected = Collected::default();
    let colour = collected.want(
        "maps/shared.dds",
        "maps/shared.tif",
        TextureMode::Colour,
        "paint",
    );
    let data = collected.want(
        "maps/shared.dds",
        "maps/shared.tif",
        TextureMode::Data,
        "mask",
    );
    let repeated = collected.want(
        "Maps/Shared.dds",
        "Maps/Shared.tif",
        TextureMode::Colour,
        "trim",
    );
    let normal = TextureMode::Normal { gloss: true };
    let normal_position = collected.want("maps/shared.dds", "maps/shared.tif", normal, "normal");

    assert_eq!(
        colour, repeated,
        "equal processing and path share one artifact"
    );
    assert_ne!(
        colour, data,
        "colour and data receive different image datablocks"
    );
    assert_ne!(
        data, normal_position,
        "the normal green transform gets its own artifact"
    );
    assert_eq!(
        collected.position_of("maps/shared.dds", normal),
        Some(normal_position)
    );
}
