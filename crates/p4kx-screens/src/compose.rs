// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::io::Write;

use p4kx_dataforge::{DataForge, Guid};
use p4kx_gfx::{Font, Localise, Raster, RasterOptions, RgbaImage, Scene, SvgReport};

use crate::canvas::{CanvasStore, StyleRef, find_canvas};
use crate::colour::{Palette, slot_names};
use crate::context::Context;
use crate::error::ScreenError;
use crate::images::{ImageSources, Images};
use crate::limits::Limits;
use crate::paint::Area;
use crate::report::{Report, VariableInForce};
use crate::style::TagPaths;
use crate::text::Texts;
use crate::variables::{Variable, check_variable_count};
use crate::walk::Walk;
use crate::widget::CanvasSizing;

const STYLES: &str = "libs/foundry/records/ui/buildingblocks/styles";
const DEFAULT_STYLE: &str = "libs/foundry/records/ui/buildingblocks/styles/default";
const MAX_CANVAS_SIDE: f32 = 16_384.0;
const CLIP_LIMIT: &str = "clipping stopped at the 24th mask: later content is not clipped";

/// A screen composed into a scene, with what the composition left out.
#[derive(Debug)]
pub struct Composed {
    scene: Scene,
    bitmaps: HashMap<u16, RgbaImage>,
    report: Report,
    width: u32,
    height: u32,
}

impl Composed {
    /// Closes the composition that `cx` took part in: the canvas `path` of `size` pixels, with
    /// the `variables` that were set.
    fn finish(
        cx: Context<'_>,
        variables: Vec<VariableInForce>,
        (path, [width, height]): (&str, [f32; 2]),
    ) -> Self {
        let Context {
            painter,
            mut report,
            images,
            ..
        } = cx;
        report.shapes = painter.len();
        report.truncated |= painter.is_full();
        report.variables = variables;
        if painter.state.mask_limit_hit {
            report.note("Overflow", CLIP_LIMIT, path);
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the sides are checked to be from 1 to 16384"
        )]
        let (pixels_wide, pixels_high) = (width.round() as u32, height.round() as u32);
        Self {
            scene: painter.finish(width, height),
            bitmaps: images.into_bitmaps(),
            report,
            width: pixels_wide,
            height: pixels_high,
        }
    }

    /// Returns the width of the screen in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height of the screen in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the drawing instructions the GFx renderer draws.
    #[must_use]
    pub const fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Returns what was drawn and what was left out.
    #[must_use]
    pub const fn report(&self) -> &Report {
        &self.report
    }

    /// Rasterises the scene.
    ///
    /// # Errors
    ///
    /// Returns [`ScreenError::Gfx`] when the canvas size the options ask for cannot be drawn.
    pub fn rasterize(&self, options: &RasterOptions) -> Result<Raster, ScreenError> {
        Ok(self.scene.rasterize(&self.bitmaps, options)?)
    }

    /// Writes the scene as an SVG document.
    ///
    /// # Errors
    ///
    /// Returns [`ScreenError::Gfx`] when the canvas size cannot be drawn or `out` fails.
    pub fn write_svg<W: Write>(
        &self,
        options: &RasterOptions,
        title: Option<&str>,
        out: W,
    ) -> Result<SvgReport, ScreenError> {
        Ok(self.scene.write_svg(&self.bitmaps, options, title, out)?)
    }
}

/// The fonts and the language the texts of screens are drawn with.
///
/// `fonts` are those of the movie `fonts_en.gfx`, which the `UIElement` Record
/// `uielements/buildingblocks` names; a font style of a widget finds its font there by name.
/// `localise` looks a localisation key up, without its `@`, in the language to draw in; without
/// it keys are drawn as they are.
#[derive(Clone, Copy)]
pub struct TextSources<'a> {
    /// The fonts texts are drawn with.
    pub fonts: &'a [Font],
    /// Looks a localisation key up.
    pub localise: Option<Localise<'a>>,
}

/// Composes `BuildingBlocks` canvases of one DataForge into scenes.
pub struct ScreenComposer<'a, 'f> {
    pub(crate) store: CanvasStore<'a, 'f>,
    slots: Vec<String>,
    tags: TagPaths,
    limits: Limits,
    variables: Vec<Variable>,
    texts: Option<TextSources<'a>>,
    images: Option<ImageSources<'a>>,
}

impl<'a, 'f> ScreenComposer<'a, 'f> {
    /// Creates a composer that reads canvases and styles from `forge`.
    #[must_use]
    pub fn new(forge: &'a DataForge<'f>) -> Self {
        Self {
            store: CanvasStore::new(forge),
            slots: slot_names(forge),
            tags: TagPaths::build(forge),
            limits: Limits::DEFAULT,
            variables: Vec::new(),
            texts: None,
            images: None,
        }
    }

    /// Draws the texts of text widgets with `texts`; without it they are left out and listed.
    #[must_use]
    pub const fn with_texts(mut self, texts: TextSources<'a>) -> Self {
        self.texts = Some(texts);
        self
    }

    /// Draws the textures and vector graphics of image, icon and custom shape widgets, read with
    /// `images`; without it they are left out and listed.
    #[must_use]
    pub const fn with_images(mut self, images: ImageSources<'a>) -> Self {
        self.images = Some(images);
        self
    }

    /// Replaces the bounds of the compositions with limits.
    #[must_use]
    pub const fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// Evaluates the bindings with the game variables `variables` instead of zero; a variable
    /// that is set twice takes its last value, and one that no canvas reads is listed in the
    /// report.
    #[must_use]
    pub fn with_variables(mut self, variables: &[Variable]) -> Self {
        self.variables = variables.to_vec();
        self
    }

    /// Composes the canvas that `canvas` names, a GUID or a record path, into a scene of its own
    /// size.
    ///
    /// The colour slots (`Base`, `Accent1` and so on) of the widgets take their colours from the
    /// default style, overlaid by the HUD style of the manufacturer `brand` (a code such as
    /// `aegs`), overlaid by the styles each canvas names for itself and its nested canvases; a
    /// canvas uses its brand style of `brand`, or none when it has no style of that code, and
    /// without a `brand` its first brand style. The style entries of those canvases and styles,
    /// whose selectors the composer evaluates, set the colours, borders, alpha, visibility and
    /// geometry of the widgets at rest. The bindings of each canvas, calculated with every game
    /// variable at zero, decide which widgets are instantiated and active, unless
    /// [`ScreenComposer::with_variables`] sets some.
    ///
    /// # Errors
    ///
    /// Returns [`ScreenError::UnknownCanvas`], [`ScreenError::AmbiguousCanvas`] or
    /// [`ScreenError::NotACanvas`] when `canvas` names no canvas, and
    /// [`ScreenError::EmptyCanvas`] when a side is less than 1 pixel and
    /// [`ScreenError::CanvasTooLarge`] when one is more than 16384, and
    /// [`ScreenError::TooManyVariables`] when more than [`crate::MAX_VARIABLES`] are set.
    pub fn compose(&mut self, canvas: &str, brand: Option<&str>) -> Result<Composed, ScreenError> {
        check_variable_count(self.variables.len())?;
        let guid = find_canvas(self.store.forge(), canvas)?;
        self.compose_guid(guid, brand)
    }

    fn compose_guid(&mut self, guid: Guid, brand: Option<&str>) -> Result<Composed, ScreenError> {
        let root = self
            .store
            .get(guid)
            .ok_or_else(|| ScreenError::UnknownCanvas {
                text: guid.to_string(),
            })?;
        let [width, height] = root.size;
        check_size(&root.record_path, width, height)?;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the sides are checked to be from 1 to 16384"
        )]
        let side = width.max(height).ceil() as u32;
        let mut cx = self.context(brand, side);
        let mut walk = Walk::new(
            &mut self.store,
            &self.slots,
            &self.tags,
            brand,
            &mut cx,
            self.limits,
        )
        .with_variables(&self.variables);
        walk.survey(&root);
        walk.enter(
            &root,
            Area {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            1.0,
            0,
            CanvasSizing::Size,
        );
        let variables = walk.variables_in_force();
        Ok(Composed::finish(
            cx,
            variables,
            (&root.record_path, [width, height]),
        ))
    }

    fn context(&self, brand: Option<&str>, side: u32) -> Context<'a> {
        Context {
            palette: self.base_palette(brand),
            texts: self.texts.map_or_else(Texts::default, |texts| {
                Texts::new(texts.fonts, texts.localise)
            }),
            images: self.images.map_or_else(Images::default, |sources| {
                Images::new(sources, side, self.limits)
            }),
            ..Context::default()
        }
    }

    /// Returns the manufacturer code in the class name of an Entity, e.g. `aegs` for
    /// `AEGS_Avenger_Titan`, when the Archive has a HUD style for it.
    #[must_use]
    pub fn brand_of(&self, class_name: &str) -> Option<String> {
        let code = class_name.split('_').next()?.to_ascii_lowercase();
        self.style_ref(&brand_style_path(&code)).map(|_| code)
    }

    fn style_ref(&self, path: &str) -> Option<StyleRef> {
        let record = self.store.forge().records_at_path(path).next()?;
        Some(StyleRef {
            guid: record.guid(),
            record_path: path.to_owned(),
        })
    }

    fn base_palette(&self, brand: Option<&str>) -> Palette {
        let styles: Vec<StyleRef> = std::iter::once(DEFAULT_STYLE.to_owned())
            .chain(brand.map(|code| brand_style_path(&code.to_ascii_lowercase())))
            .filter_map(|path| self.style_ref(&path))
            .collect();
        let styles: Vec<&StyleRef> = styles.iter().collect();
        Palette::of_styles(self.store.forge(), &self.slots, &styles)
    }

    pub(crate) const fn store(&mut self) -> &mut CanvasStore<'a, 'f> {
        &mut self.store
    }
}

fn check_size(path: &str, width: f32, height: f32) -> Result<(), ScreenError> {
    if width > MAX_CANVAS_SIDE || height > MAX_CANVAS_SIDE {
        return Err(ScreenError::CanvasTooLarge {
            path: path.to_owned(),
            width,
            height,
        });
    }
    if !(width >= 1.0 && height >= 1.0) {
        return Err(ScreenError::EmptyCanvas {
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn brand_style_path(code: &str) -> String {
    format!("{STYLES}/s_{code}_hud")
}
