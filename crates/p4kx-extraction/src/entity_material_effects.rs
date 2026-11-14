// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_dataforge::{
    DataForge, DataForgeError, Guid, JsonLayout, RecordJson, RecordJsonObserver, RecordJsonSource,
    RecordJsonStep, StructValue, TreeBuilder, Value,
};
use serde_json::Value as JsonValue;
use thiserror::Error;

use crate::{
    DataForgeEntry, DataForgeOrigin, FieldOrigins, FieldPath, InputScope, RecordJsonLocation,
    SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUseRole, SourceUses,
};

const ENTITY: &str = "EntityClassDefinition";
const EFFECTS: &str = "SEntityComponentEffects";

/// Ordered material-effect groups of one original Entity and their physical value origins.
#[derive(Debug)]
#[non_exhaustive]
pub struct EntityMaterialEffects {
    /// The physical Entity Record index.
    pub record_index: u32,
    /// The authored Entity GUID.
    pub guid: Guid,
    /// Material effects at their original component slots.
    pub components: Vec<EntityMaterialEffectsComponent>,
    /// Shared physical inputs referenced by the component fields.
    pub provenance: SourceInventory,
}

/// The complete authored material-effects value of a component.
#[derive(Debug)]
#[non_exhaustive]
pub struct EntityMaterialEffectsComponent {
    /// Original zero-based position in the Entity's Components array.
    pub component: usize,
    /// Original declared component type.
    pub source_type: String,
    /// Canonical Record JSON value with original order, nulls, and read errors.
    pub declaration: JsonValue,
    /// Sources relative to `declaration`, with the original enclosing component path.
    pub field_origins: FieldOrigins,
}

/// An Entity source or its authored effect declarations could not be observed.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EntityMaterialEffectsError {
    /// The authenticated DataForge bytes do not own this Record.
    #[error("the DataForge input or Entity record identity differs")]
    Identity,
    /// The original Entity Record could not be read.
    #[error(transparent)]
    Record(#[from] DataForgeError),
    /// The selected declaration or its field evidence could not be emitted.
    #[error(transparent)]
    Read(#[from] io::Error),
    /// The canonical Record JSON value could not be read.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// The source inventory could not be registered.
    #[error(transparent)]
    Evidence(#[from] crate::SourceEvidenceError),
}

impl EntityMaterialEffects {
    /// Reads the material-effect declarations bound to an exact original Entity.
    ///
    /// # Errors
    /// Returns a mismatched identity, original record-layout or evidence error.
    pub fn read(
        entry: &DataForgeEntry,
        forge: &DataForge<'_>,
        record_index: u32,
        guid: Guid,
    ) -> Result<Self, EntityMaterialEffectsError> {
        let origin = entry
            .origin_of(forge)
            .ok_or(EntityMaterialEffectsError::Identity)?;
        let record = forge
            .record(record_index)
            .filter(|record| record.guid() == guid)
            .ok_or(EntityMaterialEffectsError::Identity)?;
        if record.type_name()? != ENTITY {
            return Err(EntityMaterialEffectsError::Identity);
        }
        let positioned = TreeBuilder::new().build_positioned(&record)?;
        Self::project(&positioned, origin)
    }

    fn project(
        positioned: &p4kx_dataforge::PositionedRecord<'_>,
        origin: &DataForgeOrigin,
    ) -> Result<Self, EntityMaterialEffectsError> {
        let record_index = positioned.tree().record_index();
        let uses = SourceUses::with_compact_use_refs();
        let (input, complete) = bound_source(origin);
        let scope = uses.observe(input, InputScope::Payload)?;
        let mut writer = RecordJson::new(JsonLayout::Compact);
        let mut components = Vec::new();
        if let Some((field, Value::Array(items))) = field(positioned.tree().root(), "Components") {
            for (component, item) in items.iter().enumerate() {
                let Value::Struct(structure) = item else {
                    continue;
                };
                let Some([effects, material]) = path(structure) else {
                    continue;
                };
                let ordinal = u32::try_from(component).map_err(io::Error::other)?;
                let steps = [
                    RecordJsonStep::Field(field),
                    RecordJsonStep::Element(ordinal),
                    effects,
                    material,
                ];
                let mut bytes = Vec::new();
                let mut capture = Capture::new(
                    &uses,
                    &scope,
                    complete,
                    record_index,
                    structure.struct_index(),
                )?;
                writer.write_observed_value(positioned, &steps, &mut bytes, &mut capture)?;
                components.push(EntityMaterialEffectsComponent {
                    component,
                    source_type: structure.type_name().to_owned(),
                    declaration: serde_json::from_slice(&bytes)?,
                    field_origins: capture.fields,
                });
            }
        }
        Ok(Self {
            record_index,
            guid: positioned.tree().guid(),
            components,
            provenance: uses.freeze()?,
        })
    }
}

fn bound_source(origin: &DataForgeOrigin) -> (SourceInput<'_>, bool) {
    match origin {
        DataForgeOrigin::Archive(entry) => (
            SourceInput::Archive(entry),
            entry.ancestors().all(|step| step.is_some()),
        ),
        DataForgeOrigin::File(file) => (SourceInput::File(file), true),
        _ => (SourceInput::Unknown, false),
    }
}

fn field<'tree, 'source>(
    structure: &'tree StructValue<'source>,
    name: &str,
) -> Option<(usize, &'tree Value<'source>)> {
    structure
        .fields()
        .iter()
        .enumerate()
        .find(|(_, field)| field.name() == name)
        .map(|(index, field)| (index, field.value()))
}

fn path(structure: &StructValue<'_>) -> Option<[RecordJsonStep; 2]> {
    if structure.type_name() != EFFECTS {
        return None;
    }
    let (effects, Value::Struct(value)) = field(structure, "entityEffects")? else {
        return None;
    };
    let (material, _) = field(value, "materialEffects")?;
    Some([
        RecordJsonStep::Field(effects),
        RecordJsonStep::Field(material),
    ])
}

struct Capture<'a> {
    uses: &'a SourceUses,
    scope: &'a SourceScope,
    complete: bool,
    identity: [crate::SourceUse; 3],
    fields: FieldOrigins,
}

impl<'a> Capture<'a> {
    fn new(
        uses: &'a SourceUses,
        scope: &'a SourceScope,
        complete: bool,
        index: u32,
        component_type: u32,
    ) -> Result<Self, crate::SourceEvidenceError> {
        let selector = |field: &str| {
            scope.at(
                SourceLocation::DataForge {
                    record_index: index,
                    field: field.into(),
                },
                SourceUseRole::Selector,
            )
        };
        Ok(Self {
            uses,
            scope,
            complete,
            identity: [
                selector("guid")?,
                selector("sourceType")?,
                scope.at(
                    SourceLocation::Subresource {
                        namespace: "dataforgeStructDefinition".to_owned(),
                        indices: vec![u64::from(component_type)],
                        field: "name".to_owned(),
                    },
                    SourceUseRole::Selector,
                )?,
            ],
            fields: FieldOrigins::new(),
        })
    }
}

impl RecordJsonObserver for Capture<'_> {
    fn value(
        &mut self,
        pointer: &str,
        sources: &[RecordJsonSource<'_>],
        mut complete: bool,
    ) -> io::Result<()> {
        let mut uses = self.identity.to_vec();
        for source in sources {
            if let Some(located) = RecordJsonLocation::of(*source) {
                uses.push(
                    self.scope
                        .at(located.location, located.role)
                        .map_err(io::Error::other)?,
                );
            } else {
                complete = false;
            }
        }
        let origin = self.uses.origin(&uses, complete && self.complete);
        self.fields
            .insert(FieldPath::parse(pointer).map_err(io::Error::other)?, origin)
            .map_err(io::Error::other)
    }
}
