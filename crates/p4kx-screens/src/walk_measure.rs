// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::hash::{DefaultHasher, Hash, Hasher};

use p4kx_dataforge::Guid;

use crate::canvas::Canvas;
use crate::geometry::{Dim, Geometry};
use crate::look::Look;
use crate::paint::Area;
use crate::style::Modifiers;
use crate::walk::Walk;
use crate::widget::Widget;

/// A widget measured under an ancestor chain and the modifiers of the clones in force, in a
/// parent of a size, at a pixel size.
pub(crate) type MeasureKey = (Guid, usize, u64, u64, [u32; 2], u32);

/// A widget whose content size is asked for, with what the cascade made of it.
pub(crate) struct Probe<'c> {
    pub(crate) canvas: &'c Canvas,
    pub(crate) index: usize,
    pub(crate) widget: &'c Widget,
    pub(crate) style: &'c Modifiers,
    pub(crate) geometry: &'c Geometry,
    pub(crate) kids: &'c [usize],
    pub(crate) own: usize,
    pub(crate) has_children: bool,
    pub(crate) position: usize,
}

fn edges(geometry: &Geometry, axis: usize) -> (f32, f32) {
    let (padding, margin) = (geometry.padding, geometry.margin);
    if axis == 0 {
        (padding.left + padding.right, margin.left + margin.right)
    } else {
        (padding.top + padding.bottom, margin.top + margin.bottom)
    }
}

impl Walk<'_, '_, '_, '_> {
    /// Returns the size along each axis that the content of a widget asks for where its size is
    /// automatic: a text as large as its glyphs, a container as large as its children. An axis
    /// without such a size is `None`.
    ///
    /// With `keep` the text measured is kept under the key of the widget for its paint pass;
    /// only the widgets the walk goes on to paint ask for it.
    pub(crate) fn content_of(
        &mut self,
        probe: &Probe<'_>,
        parent: Area,
        (depth, keep): (usize, bool),
    ) -> [Option<f32>; 2] {
        if !probe.geometry.size.contains(&Dim::Auto) {
            return [None; 2];
        }
        let key = self.measure_key(probe, parent);
        if let Some(known) = self.measured.get(&key) {
            return *known;
        }
        let exhausted = self.limits.widgets <= self.measures;
        self.cx.report.truncated |= exhausted;
        let size = if depth > self.limits.depth || exhausted {
            [None; 2]
        } else {
            self.measures += 1;
            self.intrinsic(probe, parent, depth, keep.then_some(key))
        };
        self.measured.insert(key, size);
        size
    }

    /// Returns whether the content size of `probe` is the size of its text, which is shaped to
    /// measure it.
    pub(crate) fn measures_text(&self, probe: &Probe<'_>) -> bool {
        probe.geometry.size.contains(&Dim::Auto)
            && !probe.has_children
            && probe.widget.text.is_some()
            && self.cx.texts.is_enabled()
    }

    pub(crate) fn measure_key(&self, probe: &Probe<'_>, parent: Area) -> MeasureKey {
        (
            probe.canvas.guid,
            probe.index,
            self.chain.last().copied().unwrap_or(0),
            {
                let mut hasher = DefaultHasher::new();
                (
                    self.override_key(),
                    self.position_key(probe.position),
                    self.bound_text(probe.canvas, probe.index),
                )
                    .hash(&mut hasher);
                hasher.finish()
            },
            [parent.width.to_bits(), parent.height.to_bits()],
            self.pixel.to_bits(),
        )
    }

    fn intrinsic(
        &mut self,
        probe: &Probe<'_>,
        parent: Area,
        depth: usize,
        keep: Option<MeasureKey>,
    ) -> [Option<f32>; 2] {
        if probe.has_children {
            return if probe.kids.is_empty() {
                [None; 2]
            } else {
                self.container_size(probe, parent, depth)
            };
        }
        let widget = probe.widget;
        if !self.measures_text(probe) {
            return [None; 2];
        }
        let font = self.font_of(widget, probe.style);
        let mut look = Look::of(widget, probe.style, self.pixel, font);
        look.bound_text = self.bound_text(probe.canvas, probe.index);
        let shaped = self.cx.texts.shape(widget, &look, parent);
        let Some(prepared) = shaped.prepared.as_ref() else {
            return [None; 2];
        };
        let Some([width, height]) = self.cx.texts.measure(prepared, parent) else {
            return [None; 2];
        };
        if let (Some(inputs), Some(input)) = (self.cx.inputs.as_mut(), prepared.input.as_ref()) {
            inputs.text_layout.insert(input.clone());
        }
        if let Some(key) = keep {
            self.shaped.insert(key, shaped);
        }
        let padding = probe.geometry.padding;
        [
            Some(width + padding.left + padding.right),
            Some(height + padding.top + padding.bottom),
        ]
    }

    fn container_size(
        &mut self,
        probe: &Probe<'_>,
        parent: Area,
        depth: usize,
    ) -> [Option<f32>; 2] {
        let place = (probe.position, probe.kids);
        let subject = self.subject_of(probe.canvas, (probe.index, probe.widget), place);
        let place = self.position_key(probe.position);
        self.push_chain(&(&subject.kind, &subject.tags, place));
        self.above.push(subject);
        let overridden = self.push_override(
            probe.canvas,
            (probe.index, probe.widget),
            (probe.kids, probe.own),
        );
        let mut extents = [Extent::default(); 2];
        let mut flowing = 0_u16;
        for (position, &kid) in probe.kids.iter().enumerate() {
            let instance = position >= probe.own;
            if instance {
                self.templates.push((probe.canvas.guid, kid));
            }
            let outer = self.outer_size(probe, (kid, position), parent, depth);
            if instance {
                self.templates.pop();
            }
            let Some(outer) = outer else {
                continue;
            };
            flowing = flowing.saturating_add(1);
            for (extent, size) in extents.iter_mut().zip(outer) {
                if let Some(size) = size {
                    extent.add(size);
                }
            }
        }
        if overridden {
            self.overrides.pop();
        }
        self.above.pop();
        self.chain.pop();
        let flex = probe.geometry.flex;
        let column = flex.is_some_and(|flex| flex.column);
        let gap = flex.map_or(0.0, |flex| {
            if column {
                flex.row_gap
            } else {
                flex.column_gap
            }
        });
        let spacing = gap * f32::from(flowing.saturating_sub(1));
        let mut size = [None; 2];
        for (axis, (slot, extent)) in size.iter_mut().zip(extents).enumerate() {
            if !extent.seen {
                continue;
            }
            let along = flex.is_some() && (axis == 1) == column;
            let inner = if along {
                extent.along + spacing
            } else {
                extent.across
            };
            *slot = Some(inner + edges(probe.geometry, axis).0);
        }
        size
    }

    /// Returns what the child `kid` of `probe` asks for along each axis, margins included, or
    /// `None` when it takes no part in the layout.
    fn outer_size(
        &mut self,
        probe: &Probe<'_>,
        (kid, position): (usize, usize),
        parent: Area,
        depth: usize,
    ) -> Option<[Option<f32>; 2]> {
        let widget = probe
            .canvas
            .widgets
            .get(kid)
            .filter(|widget| widget.in_flow && self.instantiated(kid))?;
        let plan = self.plan_kids(probe.canvas, kid, widget);
        let style = self.style_of(probe.canvas, (kid, widget), (position, &plan.kids));
        if !self.shown(kid, widget, &style) {
            return None;
        }
        let geometry = Geometry::of(widget, &style);
        let child = Probe {
            position,
            canvas: probe.canvas,
            index: kid,
            widget,
            style: &style,
            geometry: &geometry,
            has_children: !plan.kids.is_empty() || self.canvas_of(kid, widget, &style).is_some(),
            kids: &plan.kids,
            own: plan.own,
        };
        let content = self.content_of(&child, parent, (depth + 1, false));
        let mut outer = [None; 2];
        for (axis, (slot, (dim, content))) in outer
            .iter_mut()
            .zip(geometry.size.into_iter().zip(content))
            .enumerate()
        {
            let own = match dim {
                Dim::Fixed(value) => Some(value),
                Dim::Auto => content,
                Dim::Percent(_) | Dim::OfWidth(_) | Dim::OfHeight(_) => None,
            };
            *slot = own
                .filter(|own| own.is_finite())
                .map(|own| own.max(0.0) + edges(&geometry, axis).1);
        }
        Some(outer)
    }
}

/// How much of one axis the children of a container take: summed, and the largest.
#[derive(Debug, Clone, Copy, Default)]
struct Extent {
    along: f32,
    across: f32,
    seen: bool,
}

impl Extent {
    fn add(&mut self, size: f32) {
        self.along += size;
        self.across = self.across.max(size);
        self.seen = true;
    }
}
