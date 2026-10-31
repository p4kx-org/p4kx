// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The logo: its SVG files, the generated icon and PNG files, the header and the social preview.
//!
//! `P4KX_WRITE_LOGO_ASSETS=1` regenerates `crates/p4kx-gui/assets/p4kx.ico`,
//! `docs/design/p4kx-logo.png` and, with a graphics adapter, `docs/design/p4kx-social-preview.png`
//! before they are checked. `P4KX_RENDER_DIR` names a folder the rendered header is written to.

use crate::cataloging;

use std::collections::BTreeSet;
use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::{Path, PathBuf};

use cataloging::{TestResult, makers, opened};
use eframe::egui_wgpu::RendererOptions;
use egui::{Color32, CornerRadius, Rect, TextureHandle, TextureOptions, Ui, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_gui::theme::desktop::HEADER_HEIGHT;
use p4kx_gui::theme::logo::{self, ICO_SIZES, Variant};
use p4kx_gui::theme::tokens::{Weight, font};
use p4kx_gui::theme::{self, palette};
use p4kx_gui::{Model, ui};
use p4kx_testkit::wgpu_setup::software_wgpu_setup;

const LOGO_PNG_SIZE: u32 = 512;
const SOCIAL_WIDTH: u16 = 1280;
const SOCIAL_HEIGHT: u16 = 640;
const SOCIAL_MARK: u16 = 300;
const SOCIAL_WORDMARK: f32 = 190.0;
const SOCIAL_GAP: f32 = 40.0;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn design(name: &str) -> PathBuf {
    repository().join("docs/design").join(name)
}

fn icon_path() -> PathBuf {
    repository().join("crates/p4kx-gui/assets/p4kx.ico")
}

fn hex(colour: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", colour.r(), colour.g(), colour.b())
}

fn colours_of(svg: &str) -> BTreeSet<String> {
    svg.match_indices('#')
        .filter_map(|(at, _)| svg.get(at..at + 7))
        .map(str::to_owned)
        .collect()
}

fn bracket(points: [(f32, f32); 4]) -> String {
    let [first, second, third, fourth] = points;
    let (white, width) = (hex(palette::WHITE), logo::BRACKET_WIDTH);
    format!(
        r#"d="M{},{} H{} V{} H{}" fill="none" stroke="{white}" stroke-width="{width}""#,
        first.0, first.1, second.0, third.1, fourth.0
    )
}

fn cross(ends: [(f32, f32); 2], colour: Color32) -> String {
    let [from, to] = ends;
    format!(
        r#"x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-linecap="round""#,
        from.0,
        from.1,
        to.0,
        to.1,
        hex(colour),
        logo::CROSS_WIDTH
    )
}

fn mark_geometry() -> [String; 4] {
    [
        bracket(logo::LEFT_BRACKET),
        bracket(logo::RIGHT_BRACKET),
        cross(logo::BACK_STROKE, palette::ACCENT_DARK),
        cross(logo::FRONT_STROKE, palette::PRIMARY),
    ]
}

#[test]
fn the_svg_files_carry_the_geometry_and_only_theme_colours() -> TestResult {
    let drawing = BTreeSet::from([
        hex(palette::WHITE),
        hex(palette::ACCENT_DARK),
        hex(palette::PRIMARY),
    ]);
    let tile = std::fs::read_to_string(design("p4kx-logo.svg"))?;
    let mark = std::fs::read_to_string(design("p4kx-mark.svg"))?;

    let mut with_tile = drawing.clone();
    with_tile.insert(hex(palette::CANVAS));
    assert_eq!(colours_of(&tile), with_tile, "tile colours");
    assert_eq!(colours_of(&mark), drawing, "mark colours");
    let side = logo::VIEW_BOX;
    let rect = format!(
        r#"<rect width="{side}" height="{side}" rx="{}" fill="{}"/>"#,
        logo::TILE_RADIUS,
        hex(palette::CANVAS)
    );
    assert!(tile.contains(&rect), "tile");
    for svg in [&tile, &mark] {
        assert!(
            svg.contains(&format!(r#"viewBox="0 0 {side} {side}""#)),
            "view box"
        );
        for part in mark_geometry() {
            assert!(svg.contains(&part), "{part}");
        }
    }
    assert!(!mark.contains("<rect"), "the mark has no tile");
    Ok(())
}

fn encode(size: u32, rgba: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(bytes)
}

fn icon_file() -> Result<Vec<u8>, Box<dyn Error>> {
    let images = ICO_SIZES
        .iter()
        .map(|&size| encode(size, &logo::rgba(Variant::Tile, size)).map(|png| (size, png)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut file = Vec::new();
    file.extend_from_slice(&0_u16.to_le_bytes());
    file.extend_from_slice(&1_u16.to_le_bytes());
    file.extend_from_slice(&u16::try_from(images.len())?.to_le_bytes());
    let mut offset = u32::try_from(6 + 16 * images.len())?;
    for (size, png) in &images {
        let side = u8::try_from(*size).unwrap_or(0);
        file.extend_from_slice(&[side, side, 0, 0]);
        file.extend_from_slice(&1_u16.to_le_bytes());
        file.extend_from_slice(&32_u16.to_le_bytes());
        file.extend_from_slice(&u32::try_from(png.len())?.to_le_bytes());
        file.extend_from_slice(&offset.to_le_bytes());
        offset += u32::try_from(png.len())?;
    }
    for (_, png) in images {
        file.extend_from_slice(&png);
    }
    Ok(file)
}

fn decode(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), Box<dyn Error>> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info()?;
    let mut pixels = vec![0; reader.output_buffer_size().ok_or("size")?];
    let info = reader.next_frame(&mut pixels)?;
    pixels.truncate(info.buffer_size());
    Ok((info.width, info.height, pixels))
}

fn le(bytes: &[u8], at: usize, length: usize) -> Result<usize, Box<dyn Error>> {
    let field = bytes.get(at..at + length).ok_or("truncated icon file")?;
    Ok(field
        .iter()
        .rev()
        .fold(0_usize, |sum, byte| (sum << 8) | usize::from(*byte)))
}

fn write_assets_when_asked() -> TestResult {
    if std::env::var_os("P4KX_WRITE_LOGO_ASSETS").is_none() {
        return Ok(());
    }
    std::fs::write(icon_path(), icon_file()?)?;
    std::fs::write(
        design("p4kx-logo.png"),
        encode(LOGO_PNG_SIZE, &logo::rgba(Variant::Tile, LOGO_PNG_SIZE))?,
    )?;
    Ok(())
}

#[test]
fn the_committed_icon_and_png_are_the_rasterised_geometry() -> TestResult {
    write_assets_when_asked()?;

    let ico = std::fs::read(icon_path())?;
    assert_eq!(le(&ico, 0, 4)?, 0x0001_0000, "icon header");
    assert_eq!(le(&ico, 4, 2)?, ICO_SIZES.len(), "image count");
    for (index, &size) in ICO_SIZES.iter().enumerate() {
        let entry = 6 + 16 * index;
        let (length, offset) = (le(&ico, entry + 8, 4)?, le(&ico, entry + 12, 4)?);
        let png = ico
            .get(offset..offset + length)
            .ok_or("icon image out of range")?;
        let (width, height, pixels) = decode(png)?;
        assert_eq!((width, height), (size, size), "{size} px image");
        assert_eq!(pixels, logo::rgba(Variant::Tile, size), "{size} px pixels");
    }

    let (width, height, pixels) = decode(&std::fs::read(design("p4kx-logo.png"))?)?;
    assert_eq!((width, height), (LOGO_PNG_SIZE, LOGO_PNG_SIZE), "logo png");
    assert_eq!(
        pixels,
        logo::rgba(Variant::Tile, LOGO_PNG_SIZE),
        "logo png pixels"
    );
    Ok(())
}

#[test]
fn the_icon_is_legible_at_sixteen_pixels() {
    let small = logo::rgba(Variant::Tile, 16);
    let lit = small
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[0] > 0xC0 && pixel[3] == 255)
        .count();

    assert!(lit >= 20, "bright pixels of the drawing: {lit}");
}

fn save(path: &Path, width: u32, height: u32, image: &[u8]) -> TestResult {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(image)?;
    Ok(())
}

#[expect(
    clippy::print_stderr,
    reason = "tells the test run why a test checked nothing"
)]
fn renderer() -> Option<WgpuTestRenderer> {
    let state =
        catch_unwind(|| create_render_state(software_wgpu_setup(), RendererOptions::PREDICTABLE))
            .ok();
    if state.is_none() {
        eprintln!("no graphics adapter: the test checks nothing");
    }
    state.map(WgpuTestRenderer::from_render_state)
}

#[test]
fn the_header_shows_the_mark_before_the_wordmark() -> TestResult {
    let Some(renderer) = renderer() else {
        return Ok(());
    };
    let (_dir, model, _out) = opened(&makers())?;
    let mut harness = Harness::builder()
        .with_context_setup(theme::apply)
        .with_size([1000.0, 300.0])
        .renderer(renderer)
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        );
    harness.run_steps(3);

    let wordmark = harness.get_by_label("P4KX").rect();
    let image = harness.render()?;
    let white = [
        palette::WHITE.r(),
        palette::WHITE.g(),
        palette::WHITE.b(),
        255,
    ];
    let coordinate = |value: u32| f32::from(u16::try_from(value).unwrap_or(u16::MAX));
    let mark_lit = image.enumerate_pixels().any(|(x, y, pixel)| {
        pixel.0 == white && coordinate(x) < wordmark.min.x && coordinate(y) < HEADER_HEIGHT
    });
    assert!(
        mark_lit,
        "white bracket pixels in the header, left of the wordmark"
    );
    if let Some(folder) = std::env::var_os("P4KX_RENDER_DIR").map(PathBuf::from) {
        save(
            &folder.join("header-logo.png"),
            image.width(),
            image.height(),
            image.as_raw(),
        )?;
    }
    Ok(())
}

fn draw_social_preview(ui: &Ui, texture: &TextureHandle) {
    let area = ui.ctx().content_rect();
    let painter = ui.painter();
    painter.rect_filled(area, CornerRadius::ZERO, palette::CANVAS);
    let side = f32::from(SOCIAL_MARK);
    let galley = painter.layout_no_wrap(
        "P4KX".to_owned(),
        font(SOCIAL_WORDMARK, Weight::Bold),
        palette::PRIMARY,
    );
    let left = area.center().x - (side + SOCIAL_GAP + galley.size().x) / 2.0;
    let mark_rect = Rect::from_min_size(pos2(left, area.center().y - side / 2.0), vec2(side, side));
    painter.image(
        texture.id(),
        mark_rect,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    let text_at = pos2(
        mark_rect.right() + SOCIAL_GAP,
        area.center().y - galley.size().y / 2.0,
    );
    painter.galley(text_at, galley, palette::PRIMARY);
}

#[test]
fn the_social_preview_is_the_mark_and_the_wordmark_on_canvas() -> TestResult {
    let Some(renderer) = renderer() else {
        return Ok(());
    };
    let mut mark: Option<TextureHandle> = None;
    let mut harness = Harness::builder()
        .with_context_setup(theme::apply)
        .with_size([f32::from(SOCIAL_WIDTH), f32::from(SOCIAL_HEIGHT)])
        .with_pixels_per_point(1.0)
        .renderer(renderer)
        .build_ui(move |ui| {
            let texture = mark.get_or_insert_with(|| {
                ui.ctx().load_texture(
                    "social-mark",
                    logo::image(Variant::Mark, u32::from(SOCIAL_MARK)),
                    TextureOptions::LINEAR,
                )
            });
            draw_social_preview(ui, texture);
        });
    harness.run_steps(3);
    let image = harness.render()?;
    assert_eq!(
        (image.width(), image.height()),
        (u32::from(SOCIAL_WIDTH), u32::from(SOCIAL_HEIGHT)),
        "size"
    );
    assert_eq!(
        image.get_pixel(2, 2).0,
        [
            palette::CANVAS.r(),
            palette::CANVAS.g(),
            palette::CANVAS.b(),
            255
        ],
        "canvas ground"
    );
    if let Some(folder) = std::env::var_os("P4KX_RENDER_DIR").map(PathBuf::from) {
        save(
            &folder.join("social-preview.png"),
            image.width(),
            image.height(),
            image.as_raw(),
        )?;
    }
    if std::env::var_os("P4KX_WRITE_LOGO_ASSETS").is_some() {
        save(
            &design("p4kx-social-preview.png"),
            image.width(),
            image.height(),
            image.as_raw(),
        )?;
    }
    Ok(())
}
