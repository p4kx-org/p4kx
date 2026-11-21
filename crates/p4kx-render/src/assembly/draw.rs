// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::frustum::Frustum;
use super::{AssemblyScene, Item, MIN_PART_PIXELS, PartDraw};
use crate::Matrix;
use crate::scene::next_serial;

/// One instance as the GPU reads it: three rows of the part's transform, then its colour.
pub type Instance = [[f32; 4]; 4];

/// The instances of one draw group, a contiguous run of the instance list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupDraw {
    /// The index of the group in the scene.
    pub group: u32,
    /// The first instance.
    pub first: u32,
    /// The number of instances.
    pub count: u32,
}

struct Blended<'a> {
    far: f32,
    at: usize,
    item: &'a Item,
}

fn items_of<'a>(scene: &'a AssemblyScene, draw: &PartDraw) -> &'a [Item] {
    scene
        .items()
        .get(draw.items.start as usize..draw.items.end as usize)
        .unwrap_or_default()
}

const fn instance(draw: &PartDraw, item: &Item) -> Instance {
    let [first, second, third] = draw.world;
    [first, second, third, item.colour]
}

/// What one frame draws: the parts that are not hidden, inside the view and large enough, as
/// instances grouped by the mesh range and texture they share.
///
/// Opaque groups come first, one run each. Translucent items follow ordered from the part
/// farthest from the eye to the nearest, neighbours of one group sharing a run.
///
/// The work is one pass over the parts and one over their items, so the cost of a frame follows
/// the parts the view holds, never the triangles.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawList {
    serial: u64,
    instances: Vec<Instance>,
    draws: Vec<GroupDraw>,
    visible: usize,
    hidden: usize,
    outside: usize,
    small: usize,
}

/// A draw list and the physical positions of the scene parts selected for it.
#[derive(Debug)]
pub struct ObservedDrawList {
    list: DrawList,
    parts: Vec<usize>,
}

impl ObservedDrawList {
    /// Returns the original accepted scene part positions in traversal order.
    #[must_use]
    pub fn parts(&self) -> &[usize] {
        &self.parts
    }

    /// Consumes this observation and returns the draw list and accepted positions.
    #[must_use]
    pub fn into_parts(self) -> (DrawList, Vec<usize>) {
        (self.list, self.parts)
    }
}

impl DrawList {
    /// Picks the parts of `scene` that `view_proj` shows in a view `height` pixels high, leaving
    /// out those `hidden` marks, which has one entry per part of the scene.
    #[must_use]
    pub fn of(scene: &AssemblyScene, hidden: &[bool], view_proj: &Matrix, height: f32) -> Self {
        Self::select(scene, hidden, view_proj, height).0
    }

    /// Builds the same draw list while retaining the accepted scene part positions.
    #[must_use]
    pub fn of_observed(
        scene: &AssemblyScene,
        hidden: &[bool],
        view_proj: &Matrix,
        height: f32,
    ) -> ObservedDrawList {
        let (list, selected) = Self::select(scene, hidden, view_proj, height);
        let parts = selected
            .into_iter()
            .filter_map(|index| scene.draws().get(index).map(|draw| draw.part))
            .collect();
        ObservedDrawList { list, parts }
    }

    fn select(
        scene: &AssemblyScene,
        hidden: &[bool],
        view_proj: &Matrix,
        height: f32,
    ) -> (Self, Vec<usize>) {
        let frustum = Frustum::of(view_proj, height);
        let mut list = Self {
            serial: next_serial(),
            instances: Vec::new(),
            draws: Vec::new(),
            visible: 0,
            hidden: 0,
            outside: 0,
            small: 0,
        };
        let mut counts = vec![0_u32; scene.groups().len()];
        let mut chosen = Vec::new();
        let mut blended = Vec::new();
        for (at, draw) in scene.draws().iter().enumerate() {
            if hidden.get(draw.part).copied().unwrap_or(false) {
                list.hidden += 1;
            } else if !frustum.sees(draw.centre, draw.radius) {
                list.outside += 1;
            } else if frustum.pixels(draw.centre, draw.radius) < MIN_PART_PIXELS {
                list.small += 1;
            } else {
                list.visible += 1;
                chosen.push(at);
                let far = frustum.far_key(draw.centre);
                for item in items_of(scene, draw) {
                    if scene.is_translucent(item.group) {
                        blended.push(Blended { far, at, item });
                    } else if let Some(count) = counts.get_mut(item.group as usize) {
                        *count += 1;
                    }
                }
            }
        }
        list.fill(scene, &chosen, &counts);
        list.append_blended(scene, blended);
        (list, chosen)
    }

    fn fill(&mut self, scene: &AssemblyScene, chosen: &[usize], counts: &[u32]) {
        let mut next = 0_u32;
        let mut offsets = vec![0_u32; counts.len()];
        for (group, count) in counts.iter().enumerate() {
            if *count > 0 {
                self.draws.push(GroupDraw {
                    group: u32::try_from(group).unwrap_or(u32::MAX),
                    first: next,
                    count: *count,
                });
            }
            if let Some(offset) = offsets.get_mut(group) {
                *offset = next;
            }
            next += count;
        }
        self.instances = vec![[[0.0; 4]; 4]; next as usize];
        for at in chosen {
            let Some(draw) = scene.draws().get(*at) else {
                continue;
            };
            for item in items_of(scene, draw) {
                if scene.is_translucent(item.group) {
                    continue;
                }
                let Some(offset) = offsets.get_mut(item.group as usize) else {
                    continue;
                };
                if let Some(slot) = self.instances.get_mut(*offset as usize) {
                    *slot = instance(draw, item);
                }
                *offset += 1;
            }
        }
    }

    fn append_blended(&mut self, scene: &AssemblyScene, mut blended: Vec<Blended<'_>>) {
        blended.sort_by(|a, b| b.far.total_cmp(&a.far));
        for entry in blended {
            let Some(draw) = scene.draws().get(entry.at) else {
                continue;
            };
            let first = u32::try_from(self.instances.len()).unwrap_or(u32::MAX);
            self.instances.push(instance(draw, entry.item));
            match self.draws.last_mut() {
                Some(run)
                    if run.group == entry.item.group
                        && run.first.saturating_add(run.count) == first
                        && scene.is_translucent(run.group) =>
                {
                    run.count += 1;
                }
                _ => self.draws.push(GroupDraw {
                    group: entry.item.group,
                    first,
                    count: 1,
                }),
            }
        }
    }

    /// Returns a number no other draw list of this process has.
    #[must_use]
    pub const fn serial(&self) -> u64 {
        self.serial
    }

    /// Returns the instances, grouped as [`Self::draws`] says.
    #[must_use]
    pub fn instances(&self) -> &[Instance] {
        &self.instances
    }

    /// Returns the groups to draw with the run of instances of each.
    #[must_use]
    pub fn draws(&self) -> &[GroupDraw] {
        &self.draws
    }

    /// Returns the number of parts drawn.
    #[must_use]
    pub const fn visible_parts(&self) -> usize {
        self.visible
    }

    /// Returns the number of parts left out because they are hidden.
    #[must_use]
    pub const fn hidden_parts(&self) -> usize {
        self.hidden
    }

    /// Returns the number of parts left out because they lie outside the view.
    #[must_use]
    pub const fn outside_parts(&self) -> usize {
        self.outside
    }

    /// Returns the number of parts left out because they cover less than a pixel.
    #[must_use]
    pub const fn small_parts(&self) -> usize {
        self.small
    }
}
