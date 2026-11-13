// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_dataforge::Guid;
use p4kx_extraction::EntryOrigin;
use p4kx_model::{MaterialStatus, SocEntities, SocPalette, Transform};

use super::{Placement, Todo, Walker};
use crate::interior::files::stem;
use crate::interior::light::scene_light;
use crate::interior::tile_light::TileLightPlacement;
use crate::interior::{InteriorPart, InteriorPartKind};
use crate::{IssueKind, PaletteRef};

/// A part found in a `.soc` file, before it has a place in the part list.
pub(super) struct Placed<'a> {
    pub(super) name: String,
    pub(super) kind: InteriorPartKind,
    pub(super) path: Option<&'a str>,
    pub(super) transform: Transform,
    pub(super) material: Option<&'a str>,
    pub(super) palette: Option<PaletteRef>,
}

impl<'a> Placed<'a> {
    pub(super) const fn new(
        name: String,
        kind: InteriorPartKind,
        path: Option<&'a str>,
        transform: Transform,
    ) -> Self {
        Self {
            name,
            kind,
            path,
            transform,
            material: None,
            palette: None,
        }
    }
}

impl Walker<'_> {
    /// Adds the part below the part `at`, whose world transform is `world`; returns whether there
    /// was room for it.
    pub(super) fn add_placed(
        &mut self,
        todo: &Todo,
        at: usize,
        world: Transform,
        placed: Placed<'_>,
    ) -> bool {
        if !self.room(todo) {
            return false;
        }
        if placed.path.is_some() {
            self.placements.push(Placement {
                part: self.parts.len(),
                chain: Arc::clone(&todo.chain),
                container: Arc::clone(&todo.path),
                library: placed.material.map(str::to_owned),
                palette: placed.palette,
            });
        }
        self.parts.push(InteriorPart {
            name: placed.name,
            kind: placed.kind,
            parent: Some(at),
            local: placed.transform,
            world: world.then(&placed.transform),
            model_path: placed.path.map(str::to_owned),
            model: None,
            materials: Arc::new(MaterialStatus::Unnamed),
            palette: None,
            light: None,
            light_placement: None,
        });
        true
    }

    /// Adds the props and lights of a `.soc` file named `file` below the part `at`; returns
    /// whether there was room for all of them.
    pub(super) fn add_entities(
        &mut self,
        todo: &Todo,
        (at, world): (usize, Transform),
        file: &str,
        entities: &SocEntities,
        tile: Option<(&TileLightPlacement<'_>, Option<&Arc<EntryOrigin>>)>,
    ) -> bool {
        let mut invalid = entities.invalid;
        for prop in &entities.props {
            let name = if prop.name.is_empty() {
                stem(&prop.model)
            } else {
                &prop.name
            };
            let mut placed = Placed::new(
                name.to_owned(),
                InteriorPartKind::Prop,
                Some(&prop.model),
                prop.transform,
            );
            placed.material = prop.material.as_deref();
            placed.palette = prop
                .palette
                .as_ref()
                .and_then(|palette| self.entity_palette(todo, file, palette));
            if !self.add_placed(todo, at, world, placed) {
                return false;
            }
        }
        let (room, class_invalid) = self.add_class_props(todo, (at, world), file, entities);
        invalid += class_invalid;
        if !room {
            return false;
        }
        if invalid > 0 {
            self.note(
                &todo.path,
                file,
                IssueKind::EntitiesLeftOut { count: invalid },
            );
        }
        self.add_lights(todo, (at, world), file, entities, tile)
    }

    /// Adds the entities whose class Record names geometry; returns whether there was room for
    /// all of them and how many had no usable transform.
    fn add_class_props(
        &mut self,
        todo: &Todo,
        (at, world): (usize, Transform),
        file: &str,
        entities: &SocEntities,
    ) -> (bool, usize) {
        let mut invalid = 0;
        if self.classes.is_none() {
            return (true, invalid);
        }
        for entity in &entities.classed {
            let Some(geometry) = self.class_geometry(&entity.class) else {
                continue;
            };
            let Some(transform) = entity.transform else {
                invalid += 1;
                continue;
            };
            let name = if entity.name.is_empty() {
                stem(geometry.path())
            } else {
                &entity.name
            };
            let mut placed = Placed::new(
                name.to_owned(),
                InteriorPartKind::Prop,
                Some(geometry.path()),
                transform,
            );
            placed.material = entity.material.as_deref().or_else(|| geometry.material());
            placed.palette = entity.palette.as_ref().map_or_else(
                || geometry.palette().cloned(),
                |own| self.entity_palette(todo, file, own),
            );
            if !self.add_placed(todo, at, world, placed) {
                return (false, invalid);
            }
        }
        (true, invalid)
    }

    fn add_lights(
        &mut self,
        todo: &Todo,
        (at, world): (usize, Transform),
        file: &str,
        entities: &SocEntities,
        tile: Option<(&TileLightPlacement<'_>, Option<&Arc<EntryOrigin>>)>,
    ) -> bool {
        let mut unsupported: Vec<(String, usize)> = Vec::new();
        for light in &entities.lights {
            if scene_light(light).is_none() {
                match unsupported
                    .iter_mut()
                    .find(|(kind, _)| kind.eq_ignore_ascii_case(&light.light_type))
                {
                    Some((_, count)) => *count += 1,
                    None => unsupported.push((light.light_type.clone(), 1)),
                }
                continue;
            }
            let evidence = tile.and_then(|(tile, origin)| {
                let group = light.group_placement.as_deref()?;
                Some(tile.resolve(light, group, origin.cloned()))
            });
            if let Some((_, evidence)) = &evidence
                && let Some(Err(error)) = evidence.anchor
            {
                self.note(&todo.path, file, IssueKind::LightAnchor(error));
            }
            let transform = evidence
                .as_ref()
                .map_or(light.transform, |(frame, _)| *frame);
            let placed = Placed::new(light.name.clone(), InteriorPartKind::Light, None, transform);
            if !self.add_placed(todo, at, world, placed) {
                return false;
            }
            if let Some(part) = self.parts.last_mut() {
                part.light = Some(light.clone());
                part.light_placement = evidence.map(|(_, evidence)| Box::new(evidence));
            }
        }
        for (light_type, count) in unsupported {
            self.note(
                &todo.path,
                file,
                IssueKind::LightUnsupported { light_type, count },
            );
        }
        true
    }

    /// Returns the palette an entity of the file `file` chooses, or notes why it cannot.
    fn entity_palette(
        &mut self,
        todo: &Todo,
        file: &str,
        palette: &SocPalette,
    ) -> Option<PaletteRef> {
        self.classes?;
        let Ok(guid) = palette.root.parse::<Guid>() else {
            let reason = format!("the tint palette {} is no GUID", palette.root);
            self.note(&todo.path, file, IssueKind::Palette { reason });
            return None;
        };
        Some(PaletteRef {
            guid,
            child_path: palette.child.clone(),
        })
    }
}
