// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{RecordTree, StructValue};
use p4kx_extraction::SourceLocation;

use crate::read::{indexed_struct_items, struct_field, text, xyz};

const VEHICLE_PARAMS: &str = "VehicleComponentParams";
const OBJECT_CONTAINER_PARAMS: &str = "SObjectContainerComponentParams";

/// Which component of an Entity's Record names an object container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ContainerSource {
    /// `VehicleComponentParams.objectContainers`: placed at a bone of the Entity's model.
    Vehicle,
    /// `SObjectContainerComponentParams.objectContainer`: placed at the Entity's origin.
    Item,
}

impl ContainerSource {
    /// Returns the stable code of the source, e.g. `vehicle`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Vehicle => "vehicle",
            Self::Item => "item",
        }
    }
}

/// A socpak container an Entity's Record names, with where it sits relative to the Entity.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectContainerRef {
    pub(crate) file: String,
    pub(crate) bone: String,
    pub(crate) position: [f32; 3],
    pub(crate) rotation: [f32; 3],
    pub(crate) source: ContainerSource,
    pub(crate) coordinate: Option<ContainerCoordinate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContainerCoordinate {
    record: u32,
    component: usize,
    ordinal: usize,
}

impl ObjectContainerRef {
    /// Returns the original fields selecting this direct vehicle container.
    pub fn source_fields(&self) -> impl Iterator<Item = SourceLocation> + '_ {
        self.coordinate
            .into_iter()
            .map(|coordinate| {
                (
                    coordinate,
                    format!(
                        "Components[{}].objectContainers[{}]",
                        coordinate.component, coordinate.ordinal
                    ),
                )
            })
            .flat_map(|(coordinate, base)| {
                ["fileName", "boneName", "Offset.Position", "Offset.Rotation"].map(|field| {
                    SourceLocation::DataForge {
                        record_index: coordinate.record,
                        field: format!("{base}.{field}"),
                    }
                })
            })
    }

    /// Returns the container's file as the Record spells it, a path below `Data`, e.g.
    /// `objectcontainers\ships\rsi\constellation\base_int_front_main.socpak`.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Returns the bone of the Entity's model the container is placed at; empty for none.
    #[must_use]
    pub fn bone(&self) -> &str {
        &self.bone
    }

    /// Returns the offset from the bone in metres.
    #[must_use]
    pub const fn position(&self) -> [f32; 3] {
        self.position
    }

    /// Returns the rotation of the offset as `Ang3` in degrees.
    #[must_use]
    pub const fn rotation(&self) -> [f32; 3] {
        self.rotation
    }

    /// Returns the component the container comes from.
    #[must_use]
    pub const fn source(&self) -> ContainerSource {
        self.source
    }
}

/// Reads the object containers of the components of an Entity's Record, the vehicle's first.
pub(crate) fn read<'v, 'f: 'v>(
    tree: &RecordTree<'f>,
    components: impl Iterator<Item = (usize, &'v StructValue<'f>)>,
) -> Vec<ObjectContainerRef> {
    let mut found = Vec::new();
    for (component_index, component) in components {
        match component.type_name() {
            VEHICLE_PARAMS => {
                for (ordinal, container) in indexed_struct_items(component, "objectContainers") {
                    let file = text(container, "fileName").trim();
                    if file.is_empty() {
                        continue;
                    }
                    let offset = struct_field(container, "Offset");
                    found.push(ObjectContainerRef {
                        file: file.to_owned(),
                        bone: text(container, "boneName").trim().to_owned(),
                        position: xyz(offset.and_then(|o| struct_field(o, "Position"))),
                        rotation: xyz(offset.and_then(|o| struct_field(o, "Rotation"))),
                        source: ContainerSource::Vehicle,
                        coordinate: Some(ContainerCoordinate {
                            record: tree.record_index(),
                            component: component_index,
                            ordinal,
                        }),
                    });
                }
            }
            OBJECT_CONTAINER_PARAMS => {
                let file = text(component, "objectContainer").trim();
                if !file.is_empty() {
                    found.push(ObjectContainerRef {
                        file: file.to_owned(),
                        bone: String::new(),
                        position: [0.0; 3],
                        rotation: [0.0; 3],
                        source: ContainerSource::Item,
                        coordinate: None,
                    });
                }
            }
            _ => {}
        }
    }
    found
}
