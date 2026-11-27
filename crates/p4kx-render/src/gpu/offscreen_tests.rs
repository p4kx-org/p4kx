// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use super::probe::{self, Probe, TEST_ADAPTERS};
use super::{AssemblyDraw, Colours, Frame, Look, OffscreenError, Shading, Subject};
use crate::assembly::synthetic::ship;
use crate::{Camera, DrawList};

const SIZE: [u32; 2] = [96, 64];

pub(super) fn open(test: &str) -> Option<Probe> {
    probe::open(test, TEST_ADAPTERS)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a test size is far below the 2^24 an f32 holds exactly"
)]
pub(super) fn frame(size: [u32; 2]) -> Frame {
    let scene = Arc::new(ship());
    let mut camera = Camera::framing(scene.bounds());
    let aspect = size[0] as f32 / size[1].max(1) as f32;
    camera.fit(aspect);
    let uniforms = camera.uniforms(aspect);
    let hidden = vec![false; scene.parts().len()];
    let list = Arc::new(DrawList::of(
        &scene,
        &hidden,
        &uniforms.view_proj,
        size[1] as f32,
    ));
    Frame {
        subject: Subject::Assembly(AssemblyDraw { scene, list }),
        uniforms,
        size,
        look: Look::default(),
        colours: Colours::default(),
        shading: Shading::Solid,
    }
}

#[test]
fn the_ship_is_drawn_into_a_picture_of_the_wanted_size_on_the_background() {
    let Some(mut offscreen) = open("the_ship_is_drawn") else {
        return;
    };

    let Ok(picture) = offscreen.draw(&frame(SIZE)) else {
        panic!("drawing failed");
    };

    assert_eq!([picture.width(), picture.height()], SIZE, "size");
    assert_eq!(
        picture.rgba().len(),
        (SIZE[0] * SIZE[1] * 4) as usize,
        "four bytes a pixel"
    );
    let corner = picture.rgba().get(..4).unwrap_or_default();
    let background = Colours::default().background;
    for (found, wanted) in corner.iter().zip(background) {
        assert!(found.abs_diff(wanted) <= 1, "the corner is the background");
    }
    let drawn = picture
        .rgba()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|&&pixel| pixel != background)
        .count();
    assert!(drawn > 150, "the ship covers pixels, found {drawn}");
}

#[test]
fn drawing_the_same_frame_again_gives_the_same_picture() {
    let Some(mut offscreen) = open("drawing_the_same_frame_again") else {
        return;
    };
    let frame = frame(SIZE);

    let first = offscreen.draw(&frame).ok();
    let second = offscreen.draw(&frame).ok();

    assert!(first.is_some(), "drawn");
    assert_eq!(first, second, "the second read-back shows the same picture");
}

#[test]
fn an_empty_or_too_large_picture_is_refused() {
    let Some(mut offscreen) = open("an_empty_or_too_large_picture") else {
        return;
    };

    let empty = offscreen.draw(&frame([0, 64]));
    let huge = offscreen.draw(&frame([u32::MAX, 64]));

    assert!(matches!(empty, Err(OffscreenError::Empty)), "empty");
    assert!(
        matches!(huge, Err(OffscreenError::TooLarge { .. })),
        "too large"
    );
}

#[test]
fn a_picture_writes_as_a_png_that_reads_back_the_same() {
    let Some(mut offscreen) = open("a_picture_writes_as_a_png") else {
        return;
    };
    let Ok(picture) = offscreen.draw(&frame(SIZE)) else {
        panic!("drawing failed");
    };

    let mut bytes = Vec::new();
    assert!(picture.write_png(&mut bytes).is_ok(), "encoded");

    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let Ok(mut reader) = decoder.read_info() else {
        panic!("not a PNG");
    };
    let mut pixels = vec![0; reader.output_buffer_size().unwrap_or_default()];
    let info = reader.next_frame(&mut pixels);
    assert!(info.is_ok(), "decoded");
    assert_eq!(pixels, picture.rgba(), "the pixels survive");
}

#[cfg(target_os = "macos")]
#[test]
#[expect(
    clippy::print_stderr,
    reason = "records the Metal adapter identity in the native CI log"
)]
fn the_metal_adapter_draws_and_reads_back_a_frame() -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut offscreen) = open("the_metal_adapter_draws_and_reads_back_a_frame") else {
        return Ok(());
    };
    let picture = offscreen.draw(&frame(SIZE))?;
    assert_eq!(
        [picture.width(), picture.height()],
        SIZE,
        "Metal frame dimensions"
    );
    assert_eq!(
        offscreen.backend(),
        wgpu::Backend::Metal,
        "native Metal backend"
    );
    eprintln!("Metal adapter: {}", offscreen.adapter_name());
    Ok(())
}
