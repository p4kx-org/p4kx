// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "scene_definitions.rs"]
mod definitions;
#[path = "scene_composition_inputs.rs"]
mod inputs;

use std::collections::HashMap;

use self::definitions::{Definition, definitions};
pub use self::inputs::{
    ObservedScene, SceneDefinitionOrigin, SceneDrawOrigin, SceneGlyphOrigin, SceneMaskOrigin,
    SceneOrigins, ScenePlacementOrigin,
};

use crate::assets::Assets;
use crate::geometry::Rect;
use crate::morph_frame::morph_at;
use crate::place::{ColorTransform, Placement};
use crate::scene::{
    Affine, MAX_SCENE_MASKS, MAX_SCENE_PATHS, MAX_SCENE_PLACEMENTS, MAX_SCENE_RECORDS,
    MAX_SCENE_SEGMENTS, Origin, Scene, SceneItem, SceneMask, SceneShape,
};
use crate::scene_report::{MAX_SPRITE_DEPTH, SkipReason, Skipped};
use crate::scene_shape::ShapeBuilder;
use crate::scene_text::{MAX_SCENE_GLYPHS, TextState};
use crate::text_fonts::TextOptions;

/// Where the shapes of a timeline go: drawn, or into a mask.
#[derive(Clone, Copy)]
enum Target {
    Items,
    Mask(usize),
}

/// What a timeline inherits from the placement that shows it.
#[derive(Clone, Copy)]
pub(crate) struct Context {
    pub(crate) transform: Affine,
    pub(crate) color_transform: ColorTransform,
    pub(crate) mask: Option<usize>,
    target: Target,
    pub(crate) sprite: Option<u16>,
    depth: u32,
    timeline: Option<usize>,
    placement: Option<usize>,
    declaration: Option<SceneDefinitionOrigin>,
    localised: bool,
}

impl Context {
    /// Returns the context with another transform.
    pub(crate) const fn with_transform(self, transform: Affine) -> Self {
        Self { transform, ..self }
    }

    pub(crate) const fn with_localisation(self, localised: bool) -> Self {
        Self { localised, ..self }
    }
}

/// The limits of one composition.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SceneLimits {
    pub(crate) placements: usize,
    pub(crate) paths: usize,
    pub(crate) segments: usize,
    pub(crate) records: usize,
    pub(crate) masks: usize,
    pub(crate) glyphs: usize,
}

impl SceneLimits {
    pub(crate) const DEFAULT: Self = Self {
        placements: MAX_SCENE_PLACEMENTS,
        paths: MAX_SCENE_PATHS,
        segments: MAX_SCENE_SEGMENTS,
        records: MAX_SCENE_RECORDS,
        masks: MAX_SCENE_MASKS,
        glyphs: MAX_SCENE_GLYPHS,
    };
}

/// Composes the first frame of a movie into a [`Scene`].
pub(crate) struct Composer<'b> {
    pub(crate) limits: SceneLimits,
    pub(crate) records: usize,
    definitions: HashMap<u16, Definition<'b>>,
    shapes: HashMap<(u16, u16), usize>,
    pub(crate) issues: Vec<Vec<SkipReason>>,
    pub(crate) scene: Scene,
    placements: usize,
    paths: usize,
    segments: usize,
    pub(crate) stopped: bool,
    pub(crate) text: TextState<'b>,
    pub(crate) origins: Option<SceneOrigins>,
}

impl<'b> Composer<'b> {
    /// Composes the display list of the main timeline's first frame.
    pub(crate) fn compose(assets: &'b Assets<'_>, stage: Rect, options: &TextOptions<'b>) -> Scene {
        Self::compose_within(assets, stage, SceneLimits::DEFAULT, options, false).scene
    }

    pub(crate) fn compose_observed(
        assets: &'b Assets<'_>,
        stage: Rect,
        options: &TextOptions<'b>,
    ) -> ObservedScene {
        let composer = Self::compose_within(assets, stage, SceneLimits::DEFAULT, options, true);
        ObservedScene {
            scene: composer.scene,
            origins: composer.origins.unwrap_or_default(),
        }
    }

    /// Composes the display list as [`Composer::compose`] does within `limits`, without
    /// localisation or fonts beyond the movie.
    #[cfg(test)]
    pub(crate) fn compose_with(assets: &'b Assets<'_>, stage: Rect, limits: SceneLimits) -> Scene {
        Self::compose_within(assets, stage, limits, &TextOptions::default(), false).scene
    }

    fn compose_within(
        assets: &'b Assets<'_>,
        stage: Rect,
        limits: SceneLimits,
        options: &TextOptions<'b>,
        observe: bool,
    ) -> Self {
        let definitions = definitions(assets);
        let mut composer = Self {
            limits,
            records: 0,
            definitions,
            shapes: HashMap::new(),
            issues: Vec::new(),
            scene: Scene {
                stage,
                background: assets.background,
                ..Scene::default()
            },
            placements: 0,
            paths: 0,
            segments: 0,
            stopped: false,
            origins: observe.then(SceneOrigins::default),
            text: TextState::new(&assets.fonts, &assets.imports, options),
        };
        let root = Context {
            transform: Affine::IDENTITY,
            color_transform: ColorTransform::IDENTITY,
            mask: None,
            target: Target::Items,
            sprite: None,
            depth: 0,
            timeline: None,
            placement: None,
            declaration: None,
            localised: false,
        };
        composer.timeline(&assets.timeline.first_frame, root);
        composer
    }

    pub(crate) fn skip(&mut self, placement: &Placement, context: Context, reason: SkipReason) {
        self.scene.skipped.push(Skipped {
            id: placement.id,
            depth: placement.depth,
            sprite: context.sprite,
            reason,
        });
    }

    fn timeline(&mut self, placements: &[Placement], context: Context) {
        let mut masks: Vec<(usize, u16)> = Vec::new();
        for placement in placements {
            if self.stopped {
                return;
            }
            self.placements += 1;
            if self.placements > self.limits.placements {
                self.stop(placement, context, SkipReason::TooManyPlacements);
                return;
            }
            masks.retain(|&(_, clip)| clip >= placement.depth);
            if !placement.visible {
                continue;
            }
            let mask = masks.last().map(|&(mask, _)| mask).or(context.mask);
            if !placement.filters.is_empty() {
                self.skip(placement, context, SkipReason::Filters);
            }
            if placement.blend_mode > 1 {
                let mode = placement.blend_mode;
                self.skip(placement, context, SkipReason::BlendMode { mode });
            }
            let mut inner = Context {
                transform: context.transform.then(&placement.matrix.into()),
                color_transform: context.color_transform.compose(&placement.color_transform),
                mask,
                placement: self.origins.as_mut().map(|origins| {
                    origins.place(context.placement, context.timeline, placement.depth)
                }),
                ..context
            };
            if let Some(clip) = placement.clip_depth {
                if matches!(context.target, Target::Mask(_)) {
                    self.skip(placement, context, SkipReason::NestedMask);
                    continue;
                }
                if self.scene.masks.len() >= self.limits.masks {
                    self.skip(placement, context, SkipReason::TooManyMasks);
                    continue;
                }
                let index = self.scene.masks.len();
                self.scene.masks.push(SceneMask {
                    shapes: Vec::new(),
                    parent: mask,
                });
                if let Some(origins) = &mut self.origins {
                    origins.mask(inner.placement);
                }
                masks.push((index, clip));
                inner.target = Target::Mask(index);
            }
            self.place(placement, context, inner);
        }
    }

    fn place(&mut self, placement: &Placement, context: Context, mut inner: Context) {
        let definition = self.definitions.get(&placement.id).copied();
        inner.declaration = definition.and_then(Definition::origin);
        match definition {
            Some(Definition::Shape(_, shape)) => {
                let cost = shape.records.len() + shape.fill_styles.len() + shape.line_styles.len();
                let converted = self.shape(placement, context, (shape.id, 0), cost, |id| {
                    ShapeBuilder::convert(
                        id,
                        &shape.fill_styles,
                        &shape.line_styles,
                        &shape.records,
                    )
                });
                if let Some(index) = converted {
                    self.draw(placement, context, inner, index);
                }
            }
            Some(Definition::Morph(_, morph)) => {
                let cost = morph.start_records.len()
                    + morph.start_fill_styles.len()
                    + morph.start_line_styles.len();
                let converted = self.shape(
                    placement,
                    context,
                    (morph.id, placement.ratio),
                    cost,
                    |id| {
                        let (fills, lines, records) = morph_at(morph, placement.ratio);
                        ShapeBuilder::convert(id, &fills, &lines, &records)
                    },
                );
                if let Some(index) = converted {
                    self.draw(placement, context, inner, index);
                }
            }
            Some(Definition::Sprite(ordinal, sprite)) => {
                if inner.depth >= MAX_SPRITE_DEPTH {
                    self.skip(placement, context, SkipReason::SpriteDepth);
                    return;
                }
                let nested = Context {
                    sprite: Some(sprite.id),
                    timeline: Some(ordinal),
                    depth: inner.depth + 1,
                    ..inner
                };
                self.timeline(&sprite.timeline.first_frame, nested);
            }
            Some(Definition::StaticText(_, text)) => {
                self.static_text(placement, context, inner, text);
            }
            Some(Definition::EditText(_, text)) => {
                self.edit_text(placement, context, inner, text);
            }
            Some(Definition::Other(code)) => {
                self.skip(placement, context, SkipReason::NotDrawn { code });
            }
            None => self.skip(placement, context, SkipReason::UnknownDefinition),
        }
    }

    fn shape(
        &mut self,
        placement: &Placement,
        context: Context,
        (id, ratio): (u16, u16),
        cost: usize,
        convert: impl FnOnce(u16) -> (SceneShape, Vec<SkipReason>),
    ) -> Option<usize> {
        if let Some(&index) = self.shapes.get(&(id, ratio)) {
            return Some(index);
        }
        self.records = self.records.saturating_add(cost.max(1));
        if self.records > self.limits.records {
            self.stop(placement, context, SkipReason::TooManyRecords);
            return None;
        }
        let (shape, issues) = convert(id);
        let index = self.scene.shapes.len();
        self.scene.shapes.push(shape);
        self.issues.push(issues);
        self.shapes.insert((id, ratio), index);
        Some(index)
    }

    pub(crate) fn draw(
        &mut self,
        placement: &Placement,
        context: Context,
        inner: Context,
        index: usize,
    ) {
        let issues = self.issues.get(index).cloned().unwrap_or_default();
        for issue in issues {
            self.skip(placement, context, issue);
        }
        let (layers, segments) = self.scene.shapes.get(index).map_or((0, 0), |shape| {
            (
                shape.layers.len(),
                shape
                    .layers
                    .iter()
                    .map(|layer| layer.path.commands.len())
                    .sum(),
            )
        });
        self.paths = self.paths.saturating_add(layers);
        self.segments = self.segments.saturating_add(segments);
        if self.paths > self.limits.paths || self.segments > self.limits.segments {
            self.stop(placement, context, SkipReason::TooManyPaths);
            return;
        }
        if !inner.transform.is_finite() {
            self.skip(placement, context, SkipReason::InvalidGeometry);
            return;
        }
        match inner.target {
            Target::Items => {
                self.scene.items.push(SceneItem {
                    shape: index,
                    transform: inner.transform,
                    color_transform: inner.color_transform,
                    mask: inner.mask,
                    origin: Origin {
                        id: placement.id,
                        depth: placement.depth,
                        sprite: context.sprite,
                    },
                });
                if let Some(origins) = &mut self.origins {
                    origins.item(inner, index);
                }
            }
            Target::Mask(mask) => {
                if let Some(mask_shape) = self.scene.masks.get_mut(mask) {
                    mask_shape.shapes.push((index, inner.transform));
                    if let Some(origins) = &mut self.origins {
                        origins.mask_item(mask, inner, index);
                    }
                }
            }
        }
    }

    pub(crate) fn stop(&mut self, placement: &Placement, context: Context, reason: SkipReason) {
        self.stopped = true;
        self.skip(placement, context, reason);
    }
}
