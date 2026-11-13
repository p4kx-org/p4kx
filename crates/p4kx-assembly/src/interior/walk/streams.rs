// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{Todo, Walker, container_part};
use crate::IssueKind;
use crate::interior::InteriorPartKind;
use crate::interior::files::{file_name, stem};
use crate::interior::scan::StreamScan;
use crate::interior::tile_light::TileLightPlacement;

use super::entities::Placed;

impl Walker<'_> {
    pub(super) fn add_streams(
        &mut self,
        todo: &Todo,
        streams: Vec<StreamScan>,
        tile: Option<&TileLightPlacement<'_>>,
    ) {
        for stream in streams {
            if !self.room(todo) {
                return;
            }
            let Some(parent) = self.parts.get(todo.part) else {
                return;
            };
            let world = parent.world;
            let at = self.parts.len();
            let mut part = container_part(
                file_name(&stream.name).to_owned(),
                Some(todo.part),
                Transform::IDENTITY,
                world,
            );
            part.kind = InteriorPartKind::Stream;
            self.parts.push(part);
            let file = file_name(&stream.name);
            if let Some(objects) = &stream.objects {
                if objects.invalid > 0 {
                    let count = objects.invalid;
                    self.note(&todo.path, file, IssueKind::ObjectsLeftOut { count });
                }
                if let Some(kind) = objects.unsupported {
                    self.note(&todo.path, file, IssueKind::UnsupportedObjects { kind });
                }
                for object in &objects.objects {
                    let Some(path) = objects.models.get(object.model) else {
                        continue;
                    };
                    let name = stem(path).to_owned();
                    let mut placed =
                        Placed::new(name, InteriorPartKind::Model, Some(path), object.transform);
                    placed.palette = object
                        .palette
                        .and_then(|index| objects.palettes.get(index))
                        .and_then(|palette| self.palette_at(todo, file, palette));
                    if !self.add_placed(todo, at, world, placed) {
                        return;
                    }
                }
            }
            if let Some(entities) = &stream.entities
                && !self.add_entities(todo, (at, world), file, entities, tile)
            {
                return;
            }
        }
    }
}
