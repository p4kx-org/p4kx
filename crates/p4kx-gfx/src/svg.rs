// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashMap};
use std::io::{self, Write};

use base64::engine::general_purpose::STANDARD;
use base64::write::EncoderWriter;

use crate::GfxError;
use crate::geometry::Rgba;
use crate::image::RgbaImage;
use crate::raster::{Background, RasterOptions, canvas};
use crate::raster_budget::RasterLimits;
use crate::raster_paint::path;
use crate::scene::{Affine, Scene};
use crate::scene_report::Skipped;
use crate::svg_budget::Capped;
use crate::svg_layer::{Place, write_layer, write_mask_layer};
use crate::svg_plan::{Inputs, Plan};
use crate::svg_text::{coordinate, escaped, matrix};

const SVG_NAMESPACES: &str =
    "xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\"";

/// A written SVG document and what of the frame it leaves out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgReport {
    /// The width of the document in pixels.
    pub width: u32,
    /// The height of the document in pixels.
    pub height: u32,
    /// The elements skipped by the scene and by the SVG writer, in drawing order.
    pub skipped: Vec<Skipped>,
}

/// An SVG document report with the bitmaps embedded in its bytes.
#[derive(Debug)]
#[non_exhaustive]
pub struct ObservedSvgReport {
    /// Dimensions and diagnostics from the ordinary SVG writer.
    pub svg: SvgReport,
    /// Sorted bitmap IDs whose image data were emitted.
    pub bitmap_inputs: Vec<u16>,
}

impl Scene {
    /// Writes the scene to `out` as an SVG document, with the decoded `bitmaps` its bitmap fills
    /// name, by ID.
    ///
    /// The document has the size and background of [`Scene::rasterize`] for `options`. Shapes
    /// are written once as `<g>` elements and placed with `<use>` and their transforms; gradients,
    /// colour transforms, masks and strokes are written as the SVG elements and attributes that
    /// match them, and bitmaps are embedded as PNG data URIs. The same elements are skipped and
    /// the same budgets apply as for the raster, so the frame ends at the same item. A bitmap
    /// fill that does not repeat is drawn once without continuing its edge pixels, and a stroke
    /// painted with a non-repeating bitmap tiles it. The output depends only on the scene, the
    /// bitmaps and the options. `title` is escaped. The document is at most [`crate::MAX_SVG_BYTES`]
    /// long, the bound on the paths, placements and embedded bitmaps together.
    ///
    /// # Errors
    ///
    /// Returns [`GfxError::EmptyStage`] when the stage has no area, [`GfxError::CanvasSize`] when
    /// the canvas would be empty or larger than the raster allows, [`GfxError::SvgTooLarge`] when
    /// the document would pass [`crate::MAX_SVG_BYTES`], [`GfxError::Write`] when `out` fails and
    /// [`GfxError::EncodePng`] when a bitmap cannot be encoded.
    pub fn write_svg<W: Write>(
        &self,
        bitmaps: &HashMap<u16, RgbaImage>,
        options: &RasterOptions,
        title: Option<&str>,
        out: W,
    ) -> Result<SvgReport, GfxError> {
        self.write_svg_with(bitmaps, options, title, out, &RasterLimits::DEFAULT)
    }

    /// Writes the SVG and records the actual embedded bitmap inputs.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Scene::write_svg`].
    pub fn write_svg_observed<W: Write>(
        &self,
        bitmaps: &HashMap<u16, RgbaImage>,
        options: &RasterOptions,
        title: Option<&str>,
        out: W,
    ) -> Result<ObservedSvgReport, GfxError> {
        let mut bitmap_inputs = BTreeSet::new();
        let svg = self.write_svg_observing(
            bitmaps,
            options,
            title,
            out,
            &RasterLimits::DEFAULT,
            Some(&mut bitmap_inputs),
        )?;
        Ok(ObservedSvgReport {
            svg,
            bitmap_inputs: bitmap_inputs.into_iter().collect(),
        })
    }

    pub(crate) fn write_svg_with<W: Write>(
        &self,
        bitmaps: &HashMap<u16, RgbaImage>,
        options: &RasterOptions,
        title: Option<&str>,
        out: W,
        limits: &RasterLimits,
    ) -> Result<SvgReport, GfxError> {
        self.write_svg_observing(bitmaps, options, title, out, limits, None)
    }

    fn write_svg_observing<W: Write>(
        &self,
        bitmaps: &HashMap<u16, RgbaImage>,
        options: &RasterOptions,
        title: Option<&str>,
        mut out: W,
        limits: &RasterLimits,
        bitmap_inputs: Option<&mut BTreeSet<u16>>,
    ) -> Result<SvgReport, GfxError> {
        let (width, height, root) = canvas(self, options.size)?;
        let paths: Vec<Vec<Option<tiny_skia::Path>>> = self
            .shapes
            .iter()
            .map(|shape| shape.layers.iter().map(|layer| path(&layer.path)).collect())
            .collect();
        let plan = Plan::build(&Inputs {
            scene: self,
            paths: &paths,
            bitmaps,
            root,
            canvas: (width, height),
            limits,
        });
        let background = match options.background {
            Background::Transparent => None,
            Background::Movie => self.background,
            Background::Color(color) => Some(color),
        };
        let document = Document {
            scene: self,
            plan: &plan,
            paths: &paths,
            bitmaps,
            root,
            size: (width, height),
        };
        let mut capped = Capped::new(&mut out, limits.svg_bytes);
        let written = document.write(&mut capped, title, background, bitmap_inputs);
        if capped.exceeded() {
            return Err(GfxError::SvgTooLarge {
                limit: limits.svg_bytes,
            });
        }
        written.map_err(|error| match error {
            WriteError::Io(source) => GfxError::Write(source),
            WriteError::Gfx(error) => error,
        })?;
        Ok(SvgReport {
            width,
            height,
            skipped: plan.skipped,
        })
    }
}

enum WriteError {
    Io(io::Error),
    Gfx(GfxError),
}

impl From<io::Error> for WriteError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

struct Document<'a> {
    scene: &'a Scene,
    plan: &'a Plan,
    paths: &'a [Vec<Option<tiny_skia::Path>>],
    bitmaps: &'a HashMap<u16, RgbaImage>,
    root: Affine,
    size: (u32, u32),
}

impl Document<'_> {
    fn write(
        &self,
        out: &mut impl Write,
        title: Option<&str>,
        background: Option<Rgba>,
        bitmap_inputs: Option<&mut BTreeSet<u16>>,
    ) -> Result<(), WriteError> {
        let (width, height) = self.size;
        writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>")?;
        writeln!(
            out,
            "<svg {SVG_NAMESPACES} width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">"
        )?;
        if let Some(title) = title {
            out.write_all(b"<title>")?;
            escaped(out, title)?;
            out.write_all(b"</title>\n")?;
        }
        if let Some(color) = background {
            write!(
                out,
                "<rect width=\"{width}\" height=\"{height}\" fill=\"#{:02x}{:02x}{:02x}\"",
                color.red, color.green, color.blue
            )?;
            crate::svg_text::opacity(out, "fill-opacity", crate::svg_text::fraction(color.alpha))?;
            out.write_all(b"/>\n")?;
        }
        out.write_all(b"<defs>\n")?;
        self.write_images(out, bitmap_inputs)?;
        self.write_masks(out)?;
        self.write_groups(out)?;
        out.write_all(b"</defs>\n")?;
        out.write_all(b"<g")?;
        matrix(out, "transform", &self.root)?;
        out.write_all(b">\n")?;
        self.write_items(out)?;
        out.write_all(b"</g>\n</svg>\n")?;
        out.flush()?;
        Ok(())
    }

    fn write_images(
        &self,
        out: &mut impl Write,
        mut bitmap_inputs: Option<&mut BTreeSet<u16>>,
    ) -> Result<(), WriteError> {
        for (index, image) in self.plan.images.iter().enumerate() {
            let Some(source) = self.bitmaps.get(&image.id) else {
                continue;
            };
            write!(
                out,
                "<image id=\"i{index}\" width=\"{}\" height=\"{}\" xlink:href=\"data:image/png;base64,",
                image.width, image.height
            )?;
            let mut encoder = EncoderWriter::new(&mut *out, &STANDARD);
            if image.color.is_identity() {
                source.write_png(&mut encoder).map_err(WriteError::Gfx)?;
            } else {
                let mut pixels = Vec::with_capacity(source.pixels().len());
                let (rgba, _) = source.pixels().as_chunks::<4>();
                for &[red, green, blue, alpha] in rgba {
                    let color = image.color.apply(Rgba {
                        red,
                        green,
                        blue,
                        alpha,
                    });
                    pixels.extend_from_slice(&[color.red, color.green, color.blue, color.alpha]);
                }
                RgbaImage::from_pixels(source.width(), source.height(), pixels)
                    .and_then(|transformed| transformed.write_png(&mut encoder))
                    .map_err(WriteError::Gfx)?;
            }
            encoder.finish()?;
            drop(encoder);
            out.write_all(b"\"/>\n")?;
            if let Some(inputs) = bitmap_inputs.as_deref_mut() {
                inputs.insert(image.id);
            }
        }
        Ok(())
    }

    fn write_masks(&self, out: &mut impl Write) -> Result<(), WriteError> {
        let (width, height) = self.size;
        let scale = self.root.a.max(f32::EPSILON);
        for (index, used) in self.plan.masks.iter().enumerate() {
            let (true, Some(mask)) = (*used, self.scene.masks.get(index)) else {
                continue;
            };
            write!(
                out,
                "<mask id=\"m{index}\" maskUnits=\"userSpaceOnUse\" maskContentUnits=\"userSpaceOnUse\""
            )?;
            coordinate(out, "x", -self.root.tx / scale)?;
            coordinate(out, "y", -self.root.ty / scale)?;
            #[expect(
                clippy::cast_precision_loss,
                reason = "canvas sides are at most MAX_CANVAS_SIDE"
            )]
            let (extent_x, extent_y) = (width as f32 / scale, height as f32 / scale);
            coordinate(out, "width", extent_x)?;
            coordinate(out, "height", extent_y)?;
            out.write_all(b">\n")?;
            let parent = crate::raster_mask::parent(self.scene, index);
            if let Some(parent) = parent {
                writeln!(out, "<g mask=\"url(#m{parent})\">")?;
            }
            for &(shape, ref to_stage) in &mask.shapes {
                let (Some(layers), Some(outlines)) =
                    (self.scene.shapes.get(shape), self.paths.get(shape))
                else {
                    continue;
                };
                out.write_all(b"<g")?;
                matrix(out, "transform", to_stage)?;
                out.write_all(b">\n")?;
                for (layer, outline) in layers.layers.iter().zip(outlines) {
                    if outline.is_some() {
                        write_mask_layer(out, layer)?;
                    }
                }
                out.write_all(b"</g>\n")?;
            }
            if parent.is_some() {
                out.write_all(b"</g>\n")?;
            }
            out.write_all(b"</mask>\n")?;
        }
        Ok(())
    }

    fn write_groups(&self, out: &mut impl Write) -> Result<(), WriteError> {
        for (group_at, group) in self.plan.groups.iter().enumerate() {
            let Some(shape) = self.scene.shapes.get(group.shape) else {
                continue;
            };
            writeln!(out, "<g id=\"s{group_at}\">")?;
            for &(layer_at, fill) in &group.layers {
                if let Some(layer) = shape.layers.get(layer_at) {
                    let place = Place {
                        group: group_at,
                        layer: layer_at,
                    };
                    write_layer(out, place, layer, &group.color, fill, &self.plan.images)?;
                }
            }
            out.write_all(b"</g>\n")?;
        }
        Ok(())
    }

    fn write_items(&self, out: &mut impl Write) -> Result<(), WriteError> {
        let mut open: Option<usize> = None;
        for drawn in &self.plan.drawn {
            if drawn.mask != open {
                if open.is_some() {
                    out.write_all(b"</g>\n")?;
                }
                if let Some(mask) = drawn.mask {
                    writeln!(out, "<g mask=\"url(#m{mask})\">")?;
                }
                open = drawn.mask;
            }
            write!(out, "<use xlink:href=\"#s{}\"", drawn.group)?;
            if drawn.transform != Affine::IDENTITY {
                matrix(out, "transform", &drawn.transform)?;
            }
            out.write_all(b"/>\n")?;
        }
        if open.is_some() {
            out.write_all(b"</g>\n")?;
        }
        Ok(())
    }
}
