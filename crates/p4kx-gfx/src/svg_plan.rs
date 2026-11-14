// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use tiny_skia::{Path, PathStroker};

use crate::image::RgbaImage;
use crate::place::ColorTransform;
use crate::raster_budget::{Coverage, RasterLimits, covered_pixels};
use crate::raster_mask::{hairline_as_pixel, parent};
use crate::raster_paint::{key, opacity_only, transform};
use crate::scene::{Affine, LayerKind, Paint, Scene, SceneItem, ShapeLayer};
use crate::scene_report::{SkipReason, Skipped};

/// What a bitmap layer is drawn with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Fill {
    /// Solid or gradient paints, written from the scene.
    Plain,
    /// The embedded image `index`, drawn with `opacity`.
    Image { index: usize, opacity: f32 },
}

/// One image to embed: a bitmap with the colour transform applied to its pixels.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ImagePlan {
    pub(crate) id: u16,
    pub(crate) color: ColorTransform,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// The layers of a shape that are drawn for one colour transform, written once.
#[derive(Debug)]
pub(crate) struct Group {
    pub(crate) shape: usize,
    pub(crate) color: ColorTransform,
    pub(crate) layers: Vec<(usize, Fill)>,
}

/// An item that is written.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Drawn {
    pub(crate) group: usize,
    pub(crate) ordinal: usize,
    pub(crate) transform: Affine,
    pub(crate) mask: Option<usize>,
}

/// What an SVG shows, decided against the budgets of a rasterisation before anything is written.
#[derive(Debug, Default)]
pub(crate) struct Plan {
    pub(crate) groups: Vec<Group>,
    pub(crate) images: Vec<ImagePlan>,
    pub(crate) drawn: Vec<Drawn>,
    pub(crate) masks: Vec<bool>,
    pub(crate) skipped: Vec<Skipped>,
}

/// The inputs of a plan.
pub(crate) struct Inputs<'a> {
    pub(crate) scene: &'a Scene,
    pub(crate) paths: &'a [Vec<Option<Path>>],
    pub(crate) bitmaps: &'a HashMap<u16, RgbaImage>,
    pub(crate) root: Affine,
    pub(crate) canvas: (u32, u32),
    pub(crate) limits: &'a RasterLimits,
}

impl Plan {
    /// Walks the items as [`Scene::rasterize`](crate::Scene) does and charges the same budgets: the
    /// covered area of every fill, stroke and mask, and the pixels of a bitmap copy when a layer
    /// that passes the coverage needs it. A layer is planned for an item exactly when the raster
    /// draws it.
    pub(crate) fn build(inputs: &Inputs<'_>) -> Self {
        let mut builder = Builder {
            inputs,
            coverage: Coverage::new(
                inputs.canvas.0,
                inputs.canvas.1,
                inputs.limits.coverage_factor,
            ),
            copied_pixels: 0,
            plan: Self {
                masks: vec![false; inputs.scene.masks.len()],
                skipped: inputs.scene.skipped.clone(),
                ..Self::default()
            },
            group_index: HashMap::new(),
            image_index: HashMap::new(),
            layers: Vec::new(),
            exhausted: false,
        };
        for (ordinal, item) in inputs.scene.items.iter().enumerate() {
            builder.item(ordinal, item);
            if builder.exhausted {
                break;
            }
        }
        builder.plan
    }
}

struct Builder<'a> {
    inputs: &'a Inputs<'a>,
    coverage: Coverage,
    copied_pixels: u64,
    plan: Plan,
    group_index: HashMap<(usize, [u32; 8]), Vec<usize>>,
    image_index: HashMap<(u16, [u32; 8]), usize>,
    layers: Vec<(usize, Fill)>,
    exhausted: bool,
}

impl Builder<'_> {
    fn skip(&mut self, item: &SceneItem, reason: SkipReason) {
        let entry = Skipped {
            id: item.origin.id,
            depth: item.origin.depth,
            sprite: item.origin.sprite,
            reason,
        };
        if self.plan.skipped.last() != Some(&entry) {
            self.plan.skipped.push(entry);
        }
    }

    fn item(&mut self, ordinal: usize, item: &SceneItem) {
        let mask = match item.mask {
            Some(index) => match self.charge_mask(index) {
                Ok(()) => Some(index),
                Err(reason) => {
                    self.skip(item, reason);
                    self.exhausted |= reason == SkipReason::FillBudget;
                    return;
                }
            },
            None => None,
        };
        let (Some(shape), Some(outlines)) = (
            self.inputs.scene.shapes.get(item.shape),
            self.inputs.paths.get(item.shape),
        ) else {
            return;
        };
        let Some(to_canvas) = transform(&self.inputs.root.then(&item.transform)) else {
            self.skip(item, SkipReason::InvalidGeometry);
            return;
        };
        self.layers.clear();
        for (at, (layer, outline)) in shape.layers.iter().zip(outlines).enumerate() {
            let Some(outline) = outline else {
                continue;
            };
            let stroked = match &layer.kind {
                LayerKind::Fill => None,
                LayerKind::Stroke(style) => Some(style),
            };
            let (width, height) = self.inputs.canvas;
            let Some(cost) = covered_pixels(outline, to_canvas, stroked, width, height) else {
                self.skip(item, SkipReason::InvalidGeometry);
                continue;
            };
            if !self.coverage.charge(cost) {
                self.skip(item, SkipReason::FillBudget);
                self.exhausted = true;
                break;
            }
            match self.fill(layer, &item.color_transform) {
                Ok(fill) => self.layers.push((at, fill)),
                Err(reason) => self.skip(item, reason),
            }
        }
        self.place(ordinal, item, mask);
    }

    fn place(&mut self, ordinal: usize, item: &SceneItem, mask: Option<usize>) {
        if self.layers.is_empty() {
            return;
        }
        let key = (item.shape, key(&item.color_transform));
        let existing = self.group_index.get(&key).and_then(|candidates| {
            candidates.iter().copied().find(|&index| {
                self.plan
                    .groups
                    .get(index)
                    .is_some_and(|group| group.layers == self.layers)
            })
        });
        let group = existing.unwrap_or_else(|| {
            let index = self.plan.groups.len();
            self.plan.groups.push(Group {
                shape: item.shape,
                color: item.color_transform,
                layers: self.layers.clone(),
            });
            self.group_index.entry(key).or_default().push(index);
            index
        });
        self.plan.drawn.push(Drawn {
            group,
            ordinal,
            transform: item.transform,
            mask,
        });
    }

    fn fill(&mut self, layer: &ShapeLayer, color: &ColorTransform) -> Result<Fill, SkipReason> {
        match &layer.paint {
            Paint::Solid(_) => Ok(Fill::Plain),
            Paint::Gradient(gradient) => {
                transform(&gradient.transform).ok_or(SkipReason::InvalidGeometry)?;
                if gradient.stops.is_empty() {
                    return Err(SkipReason::InvalidGeometry);
                }
                Ok(Fill::Plain)
            }
            Paint::Bitmap(bitmap) => {
                transform(&bitmap.transform).ok_or(SkipReason::InvalidGeometry)?;
                let (copy, opacity) = opacity_only(color)
                    .map_or((*color, 1.0), |opacity| (ColorTransform::IDENTITY, opacity));
                let index = self.image(bitmap.bitmap_id, &copy)?;
                Ok(Fill::Image { index, opacity })
            }
        }
    }

    fn image(&mut self, id: u16, color: &ColorTransform) -> Result<usize, SkipReason> {
        let key = (id, key(color));
        if let Some(&index) = self.image_index.get(&key) {
            return Ok(index);
        }
        let image = self
            .inputs
            .bitmaps
            .get(&id)
            .ok_or(SkipReason::MissingBitmap { bitmap_id: id })?;
        let pixels = u64::from(image.width()) * u64::from(image.height());
        let total = self.copied_pixels.saturating_add(pixels);
        if total > self.inputs.limits.bitmap_pixels {
            return Err(SkipReason::BitmapMemory);
        }
        self.copied_pixels = total;
        let index = self.plan.images.len();
        self.plan.images.push(ImagePlan {
            id,
            color: *color,
            width: image.width(),
            height: image.height(),
        });
        self.image_index.insert(key, index);
        Ok(index)
    }

    fn charge_mask(&mut self, index: usize) -> Result<(), SkipReason> {
        let scene = self.inputs.scene;
        let mut chain = Vec::new();
        let mut next = Some(index);
        while let Some(at) = next {
            match self.plan.masks.get(at) {
                Some(false) => chain.push(at),
                Some(true) => break,
                None => return Err(SkipReason::MaskMemory),
            }
            next = parent(scene, at);
        }
        for &at in chain.iter().rev() {
            self.charge_one_mask(at)?;
            if let Some(slot) = self.plan.masks.get_mut(at) {
                *slot = true;
            }
        }
        Ok(())
    }

    fn charge_one_mask(&mut self, index: usize) -> Result<(), SkipReason> {
        let scene = self.inputs.scene;
        let (width, height) = self.inputs.canvas;
        let bytes = u64::from(width) * u64::from(height);
        let parent_pass = if parent(scene, index).is_some() { 2 } else { 1 };
        if !self.coverage.charge(bytes * parent_pass) {
            self.exhausted = true;
            return Err(SkipReason::FillBudget);
        }
        let shapes = scene.masks.get(index).map_or(&[][..], |mask| &mask.shapes);
        for &(shape, ref to_stage) in shapes {
            let (Some(layers), Some(outlines)) =
                (scene.shapes.get(shape), self.inputs.paths.get(shape))
            else {
                continue;
            };
            let Some(to_canvas) = transform(&self.inputs.root.then(to_stage)) else {
                continue;
            };
            for (layer, outline) in layers.layers.iter().zip(outlines) {
                let Some(outline) = outline else {
                    continue;
                };
                let scale = PathStroker::compute_resolution_scale(&to_canvas);
                let style = match &layer.kind {
                    LayerKind::Fill => None,
                    LayerKind::Stroke(style) => Some(hairline_as_pixel(style, scale)),
                };
                let Some(cost) = covered_pixels(outline, to_canvas, style.as_ref(), width, height)
                else {
                    continue;
                };
                if !self.coverage.charge(cost) {
                    self.exhausted = true;
                    return Err(SkipReason::FillBudget);
                }
            }
        }
        Ok(())
    }
}
