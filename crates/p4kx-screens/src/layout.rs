// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::rc::Rc;

use crate::flex::{Flex, Item, Slot, arrange, limit};
use crate::geometry::{Dim, Geometry};
use crate::paint::Area;
use crate::style::Modifiers;
use crate::walk_measure::MeasureKey;
use crate::widget::Widget;

pub(crate) struct Sized {
    pub(crate) size: [f32; 2],
    pub(crate) auto: bool,
    pub(crate) min: [f32; 2],
    pub(crate) max: [f32; 2],
}

fn bound(dim: Option<Dim>, parent: f32, other: f32, fallback: f32) -> f32 {
    dim.and_then(|dim| dim.along(parent, other))
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}

pub(crate) fn size_of(
    geometry: &Geometry,
    parent: [f32; 2],
    has_children: bool,
    content: [Option<f32>; 2],
) -> Sized {
    let [parent_width, parent_height] = parent;
    let [width, height] = geometry.size;
    let independent = |dim: Dim, axis: f32| match dim {
        Dim::Fixed(value) => Some(value),
        Dim::Percent(value) => Some(value * axis),
        Dim::Auto | Dim::OfWidth(_) | Dim::OfHeight(_) => None,
    };
    let auto_axis = |axis: f32, own: Option<f32>| match own {
        Some(own) => own,
        None if has_children => axis,
        None => 0.0,
    };
    let fixed_width = independent(width, parent_width);
    let fixed_height = independent(height, parent_height);
    let resolved_width = fixed_width.unwrap_or_else(|| match width {
        Dim::OfHeight(value) => value * fixed_height.unwrap_or(parent_height),
        Dim::OfWidth(value) => value * parent_width,
        _ => auto_axis(parent_width, content[0]),
    });
    let resolved_height = fixed_height.unwrap_or_else(|| match height {
        Dim::OfWidth(value) => value * resolved_width,
        Dim::OfHeight(value) => value * parent_height,
        _ => auto_axis(parent_height, content[1]),
    });
    let [min_width, min_height] = geometry.bounds.min;
    let [max_width, max_height] = geometry.bounds.max;
    let min = [
        bound(min_width, parent_width, resolved_height, 0.0),
        bound(min_height, parent_height, resolved_width, 0.0),
    ];
    let max = [
        bound(max_width, parent_width, resolved_height, f32::INFINITY),
        bound(max_height, parent_height, resolved_width, f32::INFINITY),
    ];
    let fell_back = |dim: Dim, own: Option<f32>| dim == Dim::Auto && own.is_none() && has_children;
    Sized {
        size: [
            limit(resolved_width, min[0], max[0]),
            limit(resolved_height, min[1], max[1]),
        ],
        auto: fell_back(width, content[0]) || fell_back(height, content[1]),
        min,
        max,
    }
}

/// A child to lay out: its widget, what the cascade made of it, the children it holds and the
/// size its content asks for along each axis.
pub(crate) struct Sibling<'w> {
    pub(crate) widget: &'w Widget,
    pub(crate) index: usize,
    pub(crate) has_children: bool,
    pub(crate) content: [Option<f32>; 2],
    pub(crate) text_key: Option<MeasureKey>,
    pub(crate) geometry: Geometry,
    pub(crate) style: Rc<Modifiers>,
    pub(crate) kids: Vec<usize>,
    pub(crate) own_kids: usize,
    pub(crate) instance: bool,
    pub(crate) position: usize,
}

impl Sibling<'_> {
    fn size_in(&self, parent: [f32; 2]) -> Sized {
        size_of(&self.geometry, parent, self.has_children, self.content)
    }
}

pub(crate) struct Placed {
    pub(crate) at: usize,
    pub(crate) area: Area,
    pub(crate) auto: bool,
}

fn flow_items(siblings: &[Sibling<'_>], sized: &[Sized], in_flow: &[usize]) -> Vec<Item> {
    in_flow
        .iter()
        .filter_map(|&at| Some((siblings.get(at)?, sized.get(at)?)))
        .map(|(sibling, sized)| {
            let margin = sibling.geometry.margin;
            let across = margin.left + margin.right;
            let down = margin.top + margin.bottom;
            Item {
                size: [sized.size[0] + across, sized.size[1] + down],
                grow: sibling.geometry.grow,
                shrink: sibling.geometry.shrink,
                min: [sized.min[0] + across, sized.min[1] + down],
                max: [sized.max[0] + across, sized.max[1] + down],
            }
        })
        .collect()
}

pub(crate) struct Frame<'f> {
    pub(crate) content: Area,
    pub(crate) flex: Option<&'f Flex>,
    pub(crate) centred: bool,
}

pub(crate) fn place(frame: &Frame<'_>, siblings: &[Sibling<'_>]) -> Vec<Placed> {
    let content = frame.content;
    let parent = [content.width, content.height];
    let sized: Vec<Sized> = siblings
        .iter()
        .map(|sibling| sibling.size_in(parent))
        .collect();
    let in_flow: Vec<usize> = (0..siblings.len())
        .filter(|&at| {
            frame.flex.is_some()
                && siblings
                    .get(at)
                    .is_some_and(|sibling| sibling.widget.in_flow)
        })
        .collect();
    let mut slot_of = vec![None; siblings.len()];
    for (position, &at) in in_flow.iter().enumerate() {
        if let Some(slot) = slot_of.get_mut(at) {
            *slot = Some(position);
        }
    }
    let items = flow_items(siblings, &sized, &in_flow);
    let slots = frame
        .flex
        .map_or_default(|flex| arrange(flex, parent, &items));
    siblings
        .iter()
        .zip(&sized)
        .enumerate()
        .map(|(at, (sibling, sized))| {
            let slot = slot_of
                .get(at)
                .copied()
                .flatten()
                .and_then(|position| slots.get(position));
            Placed {
                at,
                area: area_of(frame, &sibling.geometry, sized.size, slot),
                auto: sized.auto,
            }
        })
        .collect()
}

/// Returns the box of a child that is `size` large by itself: in its slot of the container, or
/// at its anchor when it is not in the flow.
fn area_of(frame: &Frame<'_>, geometry: &Geometry, size: [f32; 2], slot: Option<&Slot>) -> Area {
    let margin = geometry.margin;
    let (origin, size) = slot.map_or_else(
        || (loose_origin(frame, geometry, size), size),
        |slot| {
            (
                slot.position,
                [
                    slot.size[0] - margin.left - margin.right,
                    slot.size[1] - margin.top - margin.bottom,
                ],
            )
        },
    );
    Area {
        x: frame.content.x + origin[0] + margin.left + geometry.offset[0],
        y: frame.content.y + origin[1] + margin.top + geometry.offset[1],
        width: size[0].max(0.0),
        height: size[1].max(0.0),
    }
}

fn loose_origin(frame: &Frame<'_>, geometry: &Geometry, size: [f32; 2]) -> [f32; 2] {
    let content = frame.content;
    let reach = if frame.centred {
        [content.width / 2.0, content.height / 2.0]
    } else {
        [
            geometry.anchor[0] * content.width,
            geometry.anchor[1] * content.height,
        ]
    };
    [
        geometry.pivot[0].mul_add(-size[0], reach[0]),
        geometry.pivot[1].mul_add(-size[1], reach[1]),
    ]
}
