// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{StructValue, Value};
use p4kx_extraction::SourceLocation;

use super::origins::Position;
use super::{Geometry, GeometryComponent, GeometryVariant, model_path};
use crate::read::{indexed_struct_items, struct_field, text};

/// Geometry choices with and without an authored model.
#[derive(Default)]
pub(crate) struct GeometryVariants {
    pub(crate) models: Vec<GeometryVariant>,
    pub(crate) appearance_only: Vec<AppearanceOnlyVariant>,
}

/// Material or palette authored for a tag while reusing the base model.
#[derive(Debug, Clone)]
pub(crate) struct AppearanceOnlyVariant {
    pub(crate) tags: Vec<String>,
    pub(crate) tag_locations: Box<[SourceLocation]>,
    pub(crate) geometry: Geometry,
    pub(crate) position: Option<SourceLocation>,
    slots: Box<[usize]>,
}

impl PartialEq for AppearanceOnlyVariant {
    fn eq(&self, other: &Self) -> bool {
        self.tags == other.tags && self.geometry == other.geometry && self.slots == other.slots
    }
}

impl Eq for AppearanceOnlyVariant {}

impl Geometry {
    /// Reads tagged `SubGeometry` in breadth-first order, carrying each node's ancestor tags.
    ///
    /// With a hull base, a tagged node with only material or palette keeps the base model.
    pub(crate) fn read_variants(
        component: &StructValue<'_>,
        owner: GeometryComponent,
        hull_base: Option<&Self>,
    ) -> GeometryVariants {
        let Some(node) = struct_field(component, "Geometry") else {
            return GeometryVariants::default();
        };
        let mut variants = GeometryVariants::default();
        let mut level = vec![(
            node,
            Vec::<String>::new(),
            Vec::<SourceLocation>::new(),
            Vec::<usize>::new(),
        )];
        while !level.is_empty() {
            let mut next = Vec::new();
            for (parent, inherited, tag_sources, source) in &level {
                for (slot, sub) in indexed_struct_items(parent, "SubGeometry") {
                    let slots: Vec<_> = source.iter().copied().chain([slot]).collect();
                    let mut tags = inherited.clone();
                    let mut tag_locations = tag_sources.clone();
                    let words = text(sub, "Tags").split_whitespace();
                    if words.clone().next().is_some() {
                        tags.extend(words.map(str::to_owned));
                        tag_locations.push(
                            Position {
                                owner,
                                slots: slots.as_slice().into(),
                            }
                            .at(".Tags"),
                        );
                    }
                    variants.add(sub, owner, &slots, &tags, &tag_locations, hull_base);
                    next.push((sub, tags, tag_locations, slots));
                }
            }
            level = next;
        }
        variants
    }
}

impl GeometryVariants {
    fn add(
        &mut self,
        node: &StructValue<'_>,
        owner: GeometryComponent,
        slots: &[usize],
        tags: &[String],
        tag_locations: &[SourceLocation],
        hull_base: Option<&Geometry>,
    ) {
        let Some(data) = struct_field(node, "Geometry") else {
            return;
        };
        if let Some(path) = model_path(data) {
            self.models.push(GeometryVariant {
                tags: tags.to_vec(),
                tag_locations: tag_locations.into(),
                geometry: Geometry::from_data(data, owner, slots, path),
            });
        } else if let Some(base) =
            hull_base.filter(|_| !tags.is_empty() && permits_base_model(data))
        {
            let mut geometry = Geometry::from_data(data, owner, slots, base.path());
            if geometry.material().is_some() || geometry.palette().is_some() {
                let position = geometry.variant_position();
                geometry.origins.model.clone_from(&base.origins.model);
                self.appearance_only.push(AppearanceOnlyVariant {
                    tags: tags.to_vec(),
                    tag_locations: tag_locations.into(),
                    geometry,
                    position,
                    slots: slots.into(),
                });
            }
        }
    }
}

fn permits_base_model(data: &StructValue<'_>) -> bool {
    match data.field("Geometry") {
        None | Some(Value::Null) => true,
        Some(Value::Struct(resource)) => {
            matches!(
                resource.field("path"),
                None | Some(Value::Null | Value::String(""))
            )
        }
        _ => false,
    }
}
