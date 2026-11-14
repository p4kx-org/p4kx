// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;
use std::sync::Arc;

use super::probe::{self, Probe};
use super::{AssemblyDraw, Colours, Frame, Hologram, Look, Picture, Shading, Subject};
use crate::assembly::AssemblyScene;
use crate::assembly::synthetic::{ship, stacked};
use crate::{Camera, DrawList, HologramView};

const SIZE: [u32; 2] = [256, 192];
const DISTANCE_SCALER: f32 = 0.6;
const ALPHA_FLOOR: u8 = 8;
const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];
const CHANNEL_TOLERANCE: u8 = 10;
const DIFFERING_ONE_IN: usize = 20;
const BLESS: &str = "P4KX_BLESS_REFERENCE";
const SAVE: &str = "P4KX_RENDER_DIR";

fn frame(size: [u32; 2], background: [u8; 4], look: Hologram) -> Frame {
    frame_at(
        size,
        background,
        look,
        HologramView::default().elevation_degrees,
    )
}

fn frame_at(size: [u32; 2], background: [u8; 4], look: Hologram, elevation: f32) -> Frame {
    frame_of(Arc::new(ship()), size, background, look, elevation)
}

fn frame_of(
    scene: Arc<AssemblyScene>,
    size: [u32; 2],
    background: [u8; 4],
    look: Hologram,
    elevation: f32,
) -> Frame {
    let aspect = f64::from(size[0]) / f64::from(size[1]);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "an aspect ratio of a test size is far inside the range of an f32"
    )]
    let aspect = aspect as f32;
    let mut camera = Camera::holographic(
        scene.bounds(),
        &HologramView {
            distance_scaler: DISTANCE_SCALER,
            elevation_degrees: elevation,
            ..HologramView::default()
        },
    );
    camera.fit(aspect);
    let uniforms = camera.uniforms(aspect);
    let hidden = vec![false; scene.parts().len()];
    let list = Arc::new(DrawList::of(
        &scene,
        &hidden,
        &uniforms.view_proj,
        f32::from(u16::try_from(size[1]).unwrap_or(u16::MAX)),
    ));
    Frame {
        subject: Subject::Assembly(AssemblyDraw { scene, list }),
        uniforms,
        size,
        look: Look::default(),
        colours: Colours {
            background,
            ..Colours::default()
        },
        shading: Shading::Hologram(look),
    }
}

fn open_probe(test: &str) -> Option<Probe> {
    probe::open(test, probe::TEST_ADAPTERS)
}

fn pixel(picture: &Picture, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * picture.width() + x) * 4) as usize;
    picture
        .rgba()
        .get(at..at + 4)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .unwrap_or_default()
}

fn pixels(picture: &Picture) -> impl Iterator<Item = [u8; 4]> + '_ {
    picture.rgba().as_chunks::<4>().0.iter().copied()
}

fn drawn(look: Hologram) -> Option<Picture> {
    let mut offscreen = open_probe("the hologram")?;
    Some(
        offscreen
            .draw(&frame(SIZE, TRANSPARENT, look))
            .unwrap_or_else(|error| panic!("drawing failed: {error}")),
    )
}

#[cfg(not(target_os = "macos"))]
#[test]
#[expect(
    clippy::print_stderr,
    reason = "names the adapter in the log of the CI run"
)]
fn the_software_adapter_is_named() {
    let Some(offscreen) = open_probe("the_software_adapter_is_named") else {
        return;
    };

    eprintln!("software adapter: {}", offscreen.adapter_name());

    assert!(!offscreen.adapter_name().is_empty(), "a name");
}

#[test]
fn the_hologram_is_a_tinted_shell_on_a_transparent_background() {
    let Some(picture) = drawn(Hologram::default()) else {
        return;
    };

    assert_eq!(pixel(&picture, 0, 0), TRANSPARENT, "the corner stays clear");
    let covered: Vec<[u8; 4]> = pixels(&picture).filter(|pixel| pixel[3] > 0).collect();
    assert!(
        covered.len() > 4000,
        "the shell covers pixels: {}",
        covered.len()
    );
    let tinted = covered
        .iter()
        .filter(|[red, _, blue, _]| blue > red)
        .count();
    assert!(
        tinted * 10 > covered.len() * 9,
        "the shell is blue: {tinted} of {}",
        covered.len()
    );
    assert!(
        covered.iter().any(|pixel| pixel[3] < 200),
        "the body is transparent"
    );
}

fn eroded(mask: &[bool], [width, height]: [usize; 2]) -> Vec<bool> {
    let at = |x: usize, y: usize| mask.get(y * width + x).copied().unwrap_or(false);
    (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| {
            x > 0
                && y > 0
                && x + 1 < width
                && y + 1 < height
                && at(x, y)
                && at(x - 1, y)
                && at(x + 1, y)
                && at(x, y - 1)
                && at(x, y + 1)
        })
        .collect()
}

fn median_alpha(picture: &Picture, mask: &[bool]) -> u8 {
    let mut alphas: Vec<u8> = pixels(picture)
        .zip(mask)
        .filter(|(_, inside)| **inside)
        .map(|(pixel, _)| pixel[3])
        .collect();
    alphas.sort_unstable();
    alphas.get(alphas.len() / 2).copied().unwrap_or(0)
}

#[test]
fn the_silhouette_band_is_more_opaque_than_the_interior() {
    let Some(mut offscreen) = open_probe("the_silhouette_band") else {
        return;
    };
    let from_above = frame_at(SIZE, TRANSPARENT, Hologram::default(), 70.0);
    let Ok(picture) = offscreen.draw(&from_above) else {
        panic!("drawing failed");
    };
    let size = [picture.width() as usize, picture.height() as usize];
    let covered: Vec<bool> = pixels(&picture).map(|pixel| pixel[3] > 0).collect();
    let first = eroded(&covered, size);
    let second = eroded(&first, size);
    let band: Vec<bool> = first.iter().zip(&second).map(|(a, b)| *a && !*b).collect();
    let interior = (0..3).fold(second, |mask, _| eroded(&mask, size));

    let at_edge = median_alpha(&picture, &band);
    let inside = median_alpha(&picture, &interior);

    assert!(
        interior.iter().any(|inside| *inside),
        "the shell has an interior to measure"
    );
    assert!(
        u32::from(at_edge) * 20 > u32::from(inside) * 23,
        "the glow makes the silhouette band more opaque: {at_edge} against {inside}"
    );
}

#[test]
fn a_clear_shell_with_no_glow_draws_nothing() {
    let nothing = Hologram {
        opacity: 0.0,
        glow: 0.0,
        ..Hologram::default()
    };
    let Some(picture) = drawn(nothing) else {
        return;
    };

    assert!(
        pixels(&picture).all(|pixel| pixel == TRANSPARENT),
        "no pixel is covered"
    );
}

#[test]
fn the_layers_never_add_up_past_the_cap() {
    let Some(picture) = drawn(Hologram::default()) else {
        return;
    };
    let Hologram { opacity, glow, .. } = Hologram::default();
    let cap = 0.2_f32.mul_add(glow, opacity) * 255.0;

    let brightest = pixels(&picture).map(|pixel| pixel[3]).max().unwrap_or(0);

    assert!(
        f32::from(brightest) <= cap + 2.0,
        "the most opaque pixel is {brightest}, the cap {cap}"
    );
    assert!(brightest > 40, "the shell is visible: {brightest}");
}

#[test]
fn stacked_layers_climb_to_the_cap_and_stop_there() {
    let capped = Hologram {
        opacity: 0.5,
        glow: 0.0,
        ..Hologram::default()
    };
    let Some(mut offscreen) = open_probe("stacked_layers") else {
        return;
    };
    let piled = frame_of(
        Arc::new(stacked(400)),
        SIZE,
        TRANSPARENT,
        capped,
        HologramView::default().elevation_degrees,
    );
    let picture = offscreen
        .draw(&piled)
        .unwrap_or_else(|error| panic!("drawing failed: {error}"));
    let cap = 0.5_f32 * 255.0;

    let brightest = pixels(&picture).map(|pixel| pixel[3]).max().unwrap_or(0);

    assert!(
        f32::from(brightest) <= cap + 2.0 && f32::from(brightest) >= cap - 10.0,
        "the most opaque pixel is {brightest}, the cap {cap}"
    );
}

#[test]
fn an_opaque_background_shows_through_the_body() {
    let Some(mut offscreen) = open_probe("an_opaque_background") else {
        return;
    };
    let background = [10, 20, 30, 255];

    let Ok(picture) = offscreen.draw(&frame(SIZE, background, Hologram::default())) else {
        panic!("drawing failed");
    };
    if let Some(folder) = std::env::var_os(SAVE) {
        save(
            &picture,
            &PathBuf::from(folder).join("hologram_ship_opaque.png"),
        );
    }

    assert_eq!(
        pixel(&picture, 0, 0),
        background,
        "the corner is the background"
    );
    assert!(pixels(&picture).all(|pixel| pixel[3] == 255), "all opaque");
    assert!(
        pixels(&picture).any(|pixel| pixel != background),
        "the shell is drawn over it"
    );
}

fn reference_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/reference/hologram_ship.png")
}

fn decode(bytes: &[u8]) -> Option<Picture> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buffer).ok()?;
    buffer.truncate(info.buffer_size());
    Picture::from_rgba(info.width, info.height, buffer)
}

fn save(picture: &Picture, path: &std::path::Path) {
    let written = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::File::create(path))
        .map_err(|error| error.to_string())
        .and_then(|file| picture.write_png(file).map_err(|error| error.to_string()));
    assert!(written.is_ok(), "{}: {written:?}", path.display());
}

#[test]
fn the_ship_matches_its_reference_image_within_the_tolerance() {
    let Some(picture) = drawn(Hologram::default()) else {
        return;
    };
    if let Some(folder) = std::env::var_os(SAVE) {
        save(&picture, &PathBuf::from(folder).join("hologram_ship.png"));
    }
    if std::env::var_os(BLESS).is_some() {
        save(&picture, &reference_path());
    }

    let Some(reference) = std::fs::read(reference_path())
        .ok()
        .and_then(|bytes| decode(&bytes))
    else {
        panic!(
            "no reference image at {}; draw it with {BLESS}=1 on the software adapter",
            reference_path().display()
        );
    };

    assert_eq!(
        (reference.width(), reference.height()),
        (picture.width(), picture.height()),
        "the size of the reference"
    );
    let covered = |picture: &Picture| {
        pixels(picture)
            .filter(|pixel| pixel[3] >= ALPHA_FLOOR)
            .count()
    };
    let (wanted_coverage, found_coverage) = (covered(&reference), covered(&picture));
    assert!(
        wanted_coverage > 0
            && found_coverage * 10 >= wanted_coverage * 9
            && found_coverage * 10 <= wanted_coverage * 11,
        "the ship covers {found_coverage} pixels, the reference {wanted_coverage}"
    );
    let premultiplied = |[red, green, blue, alpha]: [u8; 4]| {
        let scale = |channel: u8| {
            u8::try_from(u16::from(channel) * u16::from(alpha) / 255).unwrap_or(u8::MAX)
        };
        [scale(red), scale(green), scale(blue), alpha]
    };
    let (mut union, mut differing) = (0_usize, 0_usize);
    for (wanted, found) in pixels(&reference).zip(pixels(&picture)) {
        if wanted[3] < ALPHA_FLOOR && found[3] < ALPHA_FLOOR {
            continue;
        }
        union += 1;
        let (wanted, found) = (premultiplied(wanted), premultiplied(found));
        differing += usize::from(
            wanted
                .iter()
                .zip(found)
                .any(|(wanted, found)| wanted.abs_diff(found) > CHANNEL_TOLERANCE),
        );
    }
    assert!(
        differing * DIFFERING_ONE_IN <= union,
        "{differing} of {union} covered pixels differ by more than {CHANNEL_TOLERANCE} in a premultiplied channel"
    );
}
