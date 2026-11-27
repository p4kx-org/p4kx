// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::RgbaImage;

use super::{axis, minify, tile_len};

#[test]
fn a_tile_is_as_large_as_one_repeat_covers_but_never_larger_than_the_source() {
    assert_eq!(tile_len(1024, 80.0, 1024), 13);
    assert_eq!(tile_len(1024, 1.0, 512), 512);
    assert_eq!(tile_len(4, 100.0, 64), 1);
}

#[test]
fn the_coordinates_wrap_around_the_tile() {
    let (first, second, share) = axis(0.0, 1.0, 4);
    assert_eq!((first, second), (3, 0));
    assert!((share - 0.5).abs() < 1e-6);
    let (first, ..) = axis(2.0, 1.0, 4);
    assert_eq!(first, 3);
}

#[test]
fn a_box_averages_the_normal_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let pixels = [
        0_u8, 0, 0, 255, 100, 200, 0, 255, 0, 0, 0, 255, 100, 200, 0, 255,
    ];
    let image = RgbaImage::new(2, 2, pixels.to_vec())?;
    let small = minify(&image, (1, 1), None)?;
    assert_eq!(small.pixels(), [50, 100, 0, 255]);
    Ok(())
}
