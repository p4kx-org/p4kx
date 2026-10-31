// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::*;

const LIGHT_MAP: [u8; 4] = [210, 210, 210, OPAQUE];
const DARK_DIFFUSE: [u8; 4] = [20, 20, 20, OPAQUE];

fn pixel(texel: [u8; 4]) -> RgbaImage {
    RgbaImage::new(1, 1, texel.to_vec()).expect("a one-texel image")
}

fn base(specular: [f32; 3]) -> [u8; 3] {
    let tints = Tints {
        diffuse: [1.0; 3],
        specular,
        decal: None,
    };
    let image = base_colour(&pixel(DARK_DIFFUSE), Some(&pixel(LIGHT_MAP)), tints, false)
        .expect("a base colour image");
    let [red, green, blue, _] = texel(&image, 0, 0);
    [red, green, blue]
}

const DECALS: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn painted(masks: [u8; 3]) -> [u8; 4] {
    let [red, green, blue] = masks;
    let image = opacity_colour(&pixel([red, green, blue, OPAQUE]), Some(DECALS))
        .expect("a base colour image");
    texel(&image, 0, 0)
}

#[test]
fn without_decal_colours_an_opacity_map_gives_white_with_its_red_as_alpha() {
    let image = opacity_colour(&pixel([64, 200, 180, OPAQUE]), None).expect("an image");
    assert_eq!(texel(&image, 0, 0), [OPAQUE, OPAQUE, OPAQUE, 64]);
}

#[test]
fn each_mask_of_a_stencil_map_takes_its_decal_colour() {
    assert_eq!(painted([OPAQUE, 0, 0]), [OPAQUE, 0, 0, OPAQUE]);
    assert_eq!(painted([0, OPAQUE, 0]), [0, OPAQUE, 0, OPAQUE]);
    assert_eq!(painted([0, 0, OPAQUE]), [0, 0, OPAQUE, OPAQUE]);
}

#[test]
fn a_texel_that_one_mask_covers_partly_takes_only_the_colour_of_that_mask() {
    assert_eq!(painted([0, 128, 0]), [0, OPAQUE, 0, 128]);
    assert_eq!(painted([0, 0, 64]), [0, 0, OPAQUE, 64]);
    assert_eq!(painted([0, 0, 0])[3], 0, "no mask, no alpha");
}

#[test]
fn the_alpha_of_a_decal_is_its_strongest_mask_and_the_later_mask_covers_the_earlier() {
    let [red, green, blue, alpha] = painted([OPAQUE, OPAQUE, 128]);
    assert_eq!(alpha, OPAQUE);
    assert_eq!(red, 0, "green covers red");
    assert!(green > 0 && blue > 0, "blue is blended over green");
    assert_eq!(painted([40, 0, 0])[3], 40);
}

#[test]
fn a_dark_specular_colour_darkens_a_light_specular_map() {
    let [red, ..] = base([0.061; 3]);
    assert!(red < 80, "the map is multiplied by the dark colour: {red}");
}

#[test]
fn a_bright_specular_colour_keeps_the_light_map_as_the_colour() {
    let [red, ..] = base([1.0; 3]);
    assert!(red.abs_diff(210) <= 1, "the map is the colour: {red}");
}

#[test]
fn a_dark_specular_colour_makes_the_metalness_of_the_map_small() {
    let image = metallic_roughness(None, Some(&pixel(LIGHT_MAP)), [0.061; 3])
        .expect("a metallic-roughness image")
        .expect("a specular map gives an image")
        .image;
    let [_, _, metallic, _] = texel(&image, 0, 0);
    assert_eq!(metallic, 0);
}
