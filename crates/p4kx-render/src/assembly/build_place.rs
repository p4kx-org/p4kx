// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use super::build::SharedMesh;
use super::pieces::Piece;
use super::surface::{ResolvedSurface, SurfaceKey, surface_key};
use super::{
    AssemblyScene, Group, Item, MAX_VIEW_ITEMS, MaterialSuppression, MaterialSuppressionEffect,
    MaterialSuppressionReason, PartDraw, PartInfo,
};
use crate::scene::{Bounds, apply};

type GroupKey = (usize, usize, Option<u32>, bool);

struct Placement<'a> {
    surfaces: &'a HashMap<SurfaceKey, ResolvedSurface>,
    textures: &'a HashMap<String, u32>,
    groups: HashMap<GroupKey, u32>,
    suppressions: Option<BTreeMap<usize, MaterialSuppressionReason>>,
}

impl AssemblyScene {
    pub(super) fn place(
        &mut self,
        pieces: &[Piece<'_>],
        (model_of, meshes): (&[Option<usize>], &[Option<SharedMesh>]),
        (surfaces, textures): (&HashMap<SurfaceKey, ResolvedSurface>, &HashMap<String, u32>),
    ) {
        let mut placement = Placement {
            surfaces,
            textures,
            groups: HashMap::new(),
            suppressions: self.material_suppressions.as_ref().map(|_| BTreeMap::new()),
        };
        for (at, part) in pieces.iter().enumerate() {
            let mesh = model_of
                .get(at)
                .copied()
                .flatten()
                .and_then(|index| Some((index, meshes.get(index)?.as_ref()?)));
            self.place_piece(at, part, mesh, &mut placement);
        }
    }

    fn place_piece(
        &mut self,
        at: usize,
        part: &Piece<'_>,
        mesh: Option<(usize, &SharedMesh)>,
        placement: &mut Placement<'_>,
    ) {
        let mut info = PartInfo {
            source: part.source,
            name: part.name.clone(),
            path: part.path.clone().into_owned(),
            parent: part.parent,
            triangles: 0,
            interior: part.interior,
        };
        let Some((id, mesh)) = mesh else {
            if part.model.is_some() {
                self.left.parts += 1;
            }
            self.parts.push(info);
            return;
        };
        let first = self.items.len();
        let pending = self.items_of(part, (id, mesh), placement);
        if pending.is_empty() {
            self.record_suppressions(at, part, placement, MaterialSuppressionEffect::WholePiece);
            self.parts.push(info);
            return;
        }
        if first + pending.len() > MAX_VIEW_ITEMS {
            self.left.parts += 1;
            self.parts.push(info);
            return;
        }
        let Some(placed) = self.include(&mesh.bounds, &part.world) else {
            self.left.parts += 1;
            self.parts.push(info);
            return;
        };
        self.record_suppressions(at, part, placement, MaterialSuppressionEffect::DrawRange);
        info.triangles = mesh.triangles;
        self.items.extend(pending);
        self.draws.push(PartDraw {
            part: at,
            mesh: id,
            world: part.world,
            centre: placed.centre(),
            radius: placed.radius(),
            bounds: placed,
            items: u32::try_from(first).unwrap_or(u32::MAX)
                ..u32::try_from(self.items.len()).unwrap_or(u32::MAX),
        });
        self.parts.push(info);
    }

    fn record_suppressions(
        &mut self,
        at: usize,
        part: &Piece<'_>,
        placement: &Placement<'_>,
        effect: MaterialSuppressionEffect,
    ) {
        if let (Some(target), Some(suppressed)) = (
            self.material_suppressions.as_mut(),
            placement.suppressions.as_ref(),
        ) {
            target.extend(
                suppressed
                    .iter()
                    .map(|(&slot, &reason)| MaterialSuppression {
                        source: part.source,
                        ordinal: at,
                        slot,
                        reason,
                        effect,
                    }),
            );
        }
    }

    fn items_of(
        &mut self,
        part: &Piece<'_>,
        (id, mesh): (usize, &SharedMesh),
        placement: &mut Placement<'_>,
    ) -> Vec<Item> {
        if let Some(suppressed) = placement.suppressions.as_mut() {
            suppressed.clear();
        }
        let mut pending = Vec::new();
        for (position, range) in mesh.ranges.iter().enumerate() {
            let key = surface_key(part, range.material);
            let Some(resolved) = placement.surfaces.get(&key) else {
                continue;
            };
            let Some(surface) = resolved.surface.as_ref() else {
                if let (Some(suppressed), Some(suppression)) =
                    (placement.suppressions.as_mut(), resolved.suppression)
                {
                    suppressed
                        .entry(suppression.slot)
                        .or_insert(suppression.reason);
                }
                continue;
            };
            let texture = surface
                .texture
                .as_ref()
                .and_then(|path| placement.textures.get(path))
                .copied();
            let next = u32::try_from(self.groups.len()).unwrap_or(u32::MAX);
            let group = *placement
                .groups
                .entry((id, position, texture, surface.translucent))
                .or_insert(next);
            if group == next {
                if let Some(inputs) = self.mesh_node_inputs.as_mut() {
                    let node = mesh
                        .mesh_node_trace
                        .as_ref()
                        .and_then(|trace| trace.ranges.get(position))
                        .copied()
                        .flatten();
                    inputs.groups.push(node);
                }
                self.groups.push(Group {
                    first_index: range.first_index,
                    index_count: range.index_count,
                    texture,
                    translucent: surface.translucent,
                });
            }
            pending.push(Item {
                library_slot: surface.library_slot,
                group,
                colour: surface.colour,
            });
        }
        pending
    }

    fn include(&mut self, local: &Bounds, world: &[[f32; 4]; 3]) -> Option<Bounds> {
        let mut placed = Bounds::EMPTY;
        for corner in 0..8_usize {
            let pick = |axis: usize| {
                let side = if corner >> axis & 1 == 1 {
                    &local.max
                } else {
                    &local.min
                };
                side.get(axis).copied().unwrap_or(0.0)
            };
            placed.include(apply(world, [pick(0), pick(1), pick(2)]));
        }
        placed.is_valid().then(|| {
            self.bounds.union(&placed);
            placed
        })
    }
}
