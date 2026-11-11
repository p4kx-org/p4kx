// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::error::Error;

use p4kx_gfx::{
    Affine, Background, BitmapPaint, CanvasSize, ColorTransform, LayerKind, Origin, Paint,
    PathCommand, RasterOptions, Rect, RgbaImage, Scene, SceneItem, ScenePath, ScenePoint,
    SceneShape, ShapeLayer, SkipReason,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const OPTIONS: RasterOptions = RasterOptions {
    size: CanvasSize::Stage,
    background: Background::Transparent,
};

fn bitmap_shape(id: u16, bitmap_id: u16, path: ScenePath) -> SceneShape {
    SceneShape {
        id,
        layers: vec![ShapeLayer {
            path,
            paint: Paint::Bitmap(BitmapPaint {
                bitmap_id,
                transform: Affine::IDENTITY,
                repeating: true,
                smoothed: false,
            }),
            kind: LayerKind::Fill,
        }],
    }
}

fn square() -> ScenePath {
    ScenePath {
        commands: vec![
            PathCommand::MoveTo(ScenePoint { x: 0.0, y: 0.0 }),
            PathCommand::LineTo(ScenePoint { x: 80.0, y: 0.0 }),
            PathCommand::LineTo(ScenePoint { x: 80.0, y: 80.0 }),
            PathCommand::LineTo(ScenePoint { x: 0.0, y: 80.0 }),
            PathCommand::Close,
        ],
    }
}

const fn placed(shape: usize, id: u16, depth: u16, translation: f32) -> SceneItem {
    SceneItem {
        shape,
        transform: Affine {
            tx: translation,
            ..Affine::IDENTITY
        },
        color_transform: ColorTransform::IDENTITY,
        mask: None,
        origin: Origin {
            id,
            depth,
            sprite: None,
        },
    }
}

fn fixture() -> TestResult<(Scene, HashMap<u16, RgbaImage>)> {
    let scene = Scene {
        stage: Rect {
            x_min: 0,
            x_max: 200,
            y_min: 0,
            y_max: 200,
        },
        shapes: vec![
            bitmap_shape(12, 2, square()),
            bitmap_shape(13, 3, square()),
            bitmap_shape(14, 4, square()),
            bitmap_shape(15, 5, square()),
            bitmap_shape(16, 6, ScenePath::default()),
        ],
        items: vec![
            placed(0, 12, 1, 0.0),
            placed(0, 12, 2, 20.0),
            placed(1, 13, 3, 10_000.0),
            placed(3, 15, 4, 0.0),
            placed(4, 16, 5, 0.0),
        ],
        ..Scene::default()
    };
    let color = RgbaImage::from_pixels(1, 1, vec![180, 40, 20, 255])?;
    let inputs = [2, 3, 4, 6]
        .into_iter()
        .map(|id| (id, color.clone()))
        .collect();
    Ok((scene, inputs))
}

#[test]
fn raster_reports_successful_on_canvas_bitmap_paints_and_keeps_the_same_pixels() -> TestResult {
    let (scene, bitmaps) = fixture()?;
    let original = scene.rasterize(&bitmaps, &OPTIONS)?;
    let observed = scene.rasterize_observed(&bitmaps, &OPTIONS)?;
    assert_eq!(scene.bitmap_ids(), [2, 3, 4, 5, 6], "prepared references");
    assert_eq!(
        observed.bitmap_inputs,
        [2],
        "only admitted on-canvas paints"
    );
    assert_eq!(
        observed.raster, original,
        "identical pixels and diagnostics"
    );
    assert!(
        observed
            .raster
            .skipped
            .iter()
            .any(|entry| { entry.reason == SkipReason::MissingBitmap { bitmap_id: 5 } }),
        "missing input keeps its ordinary diagnostic"
    );
    Ok(())
}

#[test]
fn svg_reports_the_images_emitted_even_if_one_is_positioned_outside_the_canvas() -> TestResult {
    let (scene, bitmaps) = fixture()?;
    let mut original = Vec::new();
    let prior = scene.write_svg(&bitmaps, &OPTIONS, None, &mut original)?;
    let mut output = Vec::new();
    let observed = scene.write_svg_observed(&bitmaps, &OPTIONS, None, &mut output)?;
    assert_eq!(
        observed.bitmap_inputs,
        [2, 3],
        "these images occur in the SVG bytes"
    );
    assert_eq!(observed.svg, prior, "identical dimensions and diagnostics");
    assert_eq!(output, original, "byte-for-byte output parity");
    let document = std::str::from_utf8(&output)?;
    assert_eq!(
        document.matches("data:image/png;base64,").count(),
        2,
        "actual embedded images"
    );
    Ok(())
}
