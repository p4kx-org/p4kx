// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gfx::{
    Affine, LayerKind, Paint, PathCommand, Rgba, SceneMask, ScenePath, SceneShape, ShapeLayer,
};

use crate::paint::{Area, Painter, TWIPS, rectangle};

const AXIS_ALIGNED: f32 = 1.0e-6;
const SLACK: f32 = 1.0;

/// A box as the smallest and largest x and y.
pub(crate) type Bounds = [f32; 4];

pub(crate) fn shape_bounds(shape: &SceneShape) -> Option<Bounds> {
    shape
        .layers
        .iter()
        .filter_map(|layer| bounds_of(&layer.path))
        .reduce(|a, b| {
            [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[2].max(b[2]),
                a[3].max(b[3]),
            ]
        })
}

pub(crate) fn bounds_of(path: &ScenePath) -> Option<Bounds> {
    let mut found: Option<Bounds> = None;
    let mut take = |x: f32, y: f32| {
        let bounds = found.get_or_insert([x, y, x, y]);
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].min(y);
        bounds[2] = bounds[2].max(x);
        bounds[3] = bounds[3].max(y);
    };
    for command in &path.commands {
        match command {
            PathCommand::MoveTo(point) | PathCommand::LineTo(point) => take(point.x, point.y),
            PathCommand::QuadTo(control, point) => {
                take(control.x, control.y);
                take(point.x, point.y);
            }
            PathCommand::Close => {}
        }
    }
    found
}

fn stage_bounds(bounds: Bounds, to: &Affine, reach: f32) -> Bounds {
    let corners = [
        (bounds[0], bounds[1]),
        (bounds[2], bounds[1]),
        (bounds[2], bounds[3]),
        (bounds[0], bounds[3]),
    ];
    let spread = reach * (to.a.abs() + to.b.abs() + to.c.abs() + to.d.abs());
    corners.iter().fold(
        [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ],
        |found, &(x, y)| {
            let (sx, sy) = (
                to.a.mul_add(x, to.c.mul_add(y, to.tx)),
                to.b.mul_add(x, to.d.mul_add(y, to.ty)),
            );
            [
                found[0].min(sx - spread),
                found[1].min(sy - spread),
                found[2].max(sx + spread),
                found[3].max(sy + spread),
            ]
        },
    )
}

fn in_twips(transform: Affine) -> Affine {
    Affine {
        tx: transform.tx * TWIPS,
        ty: transform.ty * TWIPS,
        ..transform
    }
}

#[derive(Debug)]
struct Clip {
    area: Area,
    transform: Affine,
    stage: Option<Bounds>,
    masks: Vec<(Option<usize>, usize)>,
}

impl Clip {
    fn new(area: Area, transform: Affine) -> Self {
        let aligned = transform.b.abs() < AXIS_ALIGNED && transform.c.abs() < AXIS_ALIGNED;
        let stage = aligned.then(|| {
            stage_bounds(
                [
                    area.x * TWIPS,
                    area.y * TWIPS,
                    (area.x + area.width) * TWIPS,
                    (area.y + area.height) * TWIPS,
                ],
                &in_twips(transform),
                0.0,
            )
        });
        Self {
            area,
            transform,
            stage,
            masks: Vec::new(),
        }
    }

    fn contains(&self, item: Bounds) -> bool {
        self.stage.is_some_and(|clip| {
            item[0] >= clip[0] - SLACK
                && item[1] >= clip[1] - SLACK
                && item[2] <= clip[2] + SLACK
                && item[3] <= clip[3] + SLACK
        })
    }
}

/// The transform and the clips every shape painted now is placed under.
#[derive(Debug, Default)]
pub(crate) struct PaintState {
    transforms: Vec<Affine>,
    clips: Vec<Clip>,
    pub(crate) masks: Vec<SceneMask>,
}

impl PaintState {
    fn current(&self) -> Affine {
        self.transforms.last().copied().unwrap_or(Affine::IDENTITY)
    }

    pub(crate) const fn is_clipped(&self) -> bool {
        !self.clips.is_empty()
    }

    /// Returns the current transform from the pixels of the layout to the twips of the stage.
    pub(crate) fn to_twips(&self) -> Affine {
        in_twips(self.current())
    }
}

impl Painter {
    /// Draws what follows under `local` as well, in the pixels of the layout; `false` and
    /// nothing changed when the result would not be finite.
    pub(crate) fn push_transform(&mut self, local: &Affine) -> bool {
        let composed = self.state.current().then(local);
        if !composed.is_finite() {
            return false;
        }
        self.state.transforms.push(composed);
        true
    }

    pub(crate) fn pop_transform(&mut self) {
        self.state.transforms.pop();
    }

    /// Clips what follows to the box `area` of the layout under the current transform.
    pub(crate) fn push_clip(&mut self, area: Area) {
        let transform = self.state.current();
        self.state.clips.push(Clip::new(area, transform));
    }

    pub(crate) fn pop_clip(&mut self) {
        self.state.clips.pop();
    }

    /// Returns the mask that clips a shape of `bounds` in its own twips, grown by `reach`,
    /// placed by `transform`; `None` when every clip in force holds it entirely.
    pub(crate) fn mask_of(
        &mut self,
        bounds: Option<&Bounds>,
        reach: f32,
        transform: &Affine,
    ) -> Option<usize> {
        if self.state.clips.is_empty() {
            return None;
        }
        let stage = bounds.map(|bounds| stage_bounds(*bounds, transform, reach));
        let mut parent = None;
        for at in 0..self.state.clips.len() {
            let inside = match (stage, self.state.clips.get(at)) {
                (Some(stage), Some(clip)) => clip.contains(stage),
                (None, _) => true,
                (_, None) => continue,
            };
            if !inside {
                parent = self.clip_mask(at, parent);
            }
        }
        parent
    }

    fn clip_mask(&mut self, at: usize, parent: Option<usize>) -> Option<usize> {
        let clip = self.state.clips.get(at)?;
        if let Some(&(_, mask)) = clip.masks.iter().find(|(outer, _)| *outer == parent) {
            return Some(mask);
        }
        let (area, transform) = (clip.area, in_twips(clip.transform));
        let commands = if area.is_empty() {
            Vec::new()
        } else {
            rectangle(area)
        };
        let shape = self.shapes.len();
        self.shapes.push(SceneShape {
            id: u16::try_from(shape % usize::from(u16::MAX)).unwrap_or(0),
            layers: vec![ShapeLayer {
                path: ScenePath { commands },
                paint: Paint::Solid(Rgba {
                    red: 255,
                    green: 255,
                    blue: 255,
                    alpha: 255,
                }),
                kind: LayerKind::Fill,
            }],
        });
        self.state.masks.push(SceneMask {
            shapes: vec![(shape, transform)],
            parent,
        });
        let mask = self.state.masks.len() - 1;
        if let Some(clip) = self.state.clips.get_mut(at) {
            clip.masks.push((parent, mask));
        }
        Some(mask)
    }
}
