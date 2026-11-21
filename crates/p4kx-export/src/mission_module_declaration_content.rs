// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::{Observed, OwnedSlots, SOURCE_TYPE};
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::location_vocabulary::RESOURCE_SLOT;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference, SourceWeakReference};

type RecordLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;
type WeakLink<'source> = Box<SourceWeakReference<'source>>;

/// The concrete authored configuration of a mission module.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ModuleType<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    data: ModuleTypeData<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum ModuleTypeData<'source> {
    Beacon,
    Mission {
        #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
        required_location_modules: SourcedValue<Vec<RecordLink<'source>>>,
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        required_resource_slot: RecordLink<'source>,
        pauses_location_npc_autospawns: SourcedValue<bool>,
    },
    Location {
        is_persistent: SourcedValue<bool>,
    },
    Authored,
}

impl<'source> ModuleType<'source> {
    /// Borrows the original concrete module type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the mission variant's required location module slots.
    #[must_use]
    pub const fn required_location_modules(
        &self,
    ) -> Option<&SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>> {
        match &self.data {
            ModuleTypeData::Mission {
                required_location_modules,
                ..
            } => Some(required_location_modules),
            _ => None,
        }
    }

    /// Borrows the mission variant's required resource slot.
    #[must_use]
    pub const fn required_resource_slot(
        &self,
    ) -> Option<&SourcedValue<Box<SourceRecordReference<'source>>>> {
        match &self.data {
            ModuleTypeData::Mission {
                required_resource_slot,
                ..
            } => Some(required_resource_slot),
            _ => None,
        }
    }

    /// Borrows the authored mission variant autospawn flag.
    #[must_use]
    pub const fn pauses_location_npc_autospawns(&self) -> Option<&SourcedValue<bool>> {
        match &self.data {
            ModuleTypeData::Mission {
                pauses_location_npc_autospawns,
                ..
            } => Some(pauses_location_npc_autospawns),
            _ => None,
        }
    }

    /// Borrows the authored location variant persistence flag.
    #[must_use]
    pub const fn is_persistent(&self) -> Option<&SourcedValue<bool>> {
        match &self.data {
            ModuleTypeData::Location { is_persistent, .. } => Some(is_persistent),
            _ => None,
        }
    }

    /// Borrows additional authored fields in source order.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }

    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let data = match fields.source_type()? {
            "ModuleDeclarationType_Beacon" => ModuleTypeData::Beacon,
            "ModuleDeclarationType_Mission" => ModuleTypeData::Mission {
                required_location_modules: fields.record_link_array_to(
                    "requiredLocationModules",
                    SOURCE_TYPE,
                    notes,
                )?,
                required_resource_slot: fields.record_link_to(
                    "requiredResourceSlot",
                    RESOURCE_SLOT,
                    notes,
                )?,
                pauses_location_npc_autospawns: fields
                    .boolean("pausesLocationNPCAutospawns", notes)?,
            },
            "ModuleDeclarationType_Location" => ModuleTypeData::Location {
                is_persistent: fields.boolean("isPersistent", notes)?,
            },
            _ => {
                fields.note_retained_variant(notes)?;
                ModuleTypeData::Authored
            }
        };
        let projected = data.projected();
        Ok(Self {
            source_type: fields.source_type_value()?,
            authored_fields: fields.authored_members_except(projected.into_iter().flatten())?,
            data,
        })
    }
}

impl ModuleTypeData<'_> {
    const fn projected(&self) -> [Option<(&'static str, SourceValueState)>; 3] {
        match self {
            Self::Mission {
                required_location_modules,
                required_resource_slot,
                pauses_location_npc_autospawns,
            } => [
                Some(("requiredLocationModules", required_location_modules.state())),
                Some(("requiredResourceSlot", required_resource_slot.state())),
                Some((
                    "pausesLocationNPCAutospawns",
                    pauses_location_npc_autospawns.state(),
                )),
            ],
            Self::Location { is_persistent } => {
                [Some(("isPersistent", is_persistent.state())), None, None]
            }
            Self::Beacon | Self::Authored => [None; 3],
        }
    }
}

/// An authored module spawn declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ModuleSpawnedEntities<'source> {
    source_type: SourcedValue<&'source str>,
    debug_name: SourcedValue<&'source str>,
    spawn_immediately: SourcedValue<bool>,
    #[serde(flatten)]
    data: ModuleSpawnData<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum ModuleSpawnData<'source> {
    Npc {
        spawn_description_property_name: SourcedValue<&'source str>,
    },
    Authored,
}

impl<'source> ModuleSpawnedEntities<'source> {
    /// Borrows the original concrete spawn type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the authored debug name.
    #[must_use]
    pub const fn debug_name(&self) -> &SourcedValue<&'source str> {
        &self.debug_name
    }

    /// Borrows the authored immediate-spawn flag.
    #[must_use]
    pub const fn spawn_immediately(&self) -> &SourcedValue<bool> {
        &self.spawn_immediately
    }

    /// Borrows the NPC spawn property selector.
    #[must_use]
    pub const fn spawn_description_property_name(&self) -> Option<&SourcedValue<&'source str>> {
        match &self.data {
            ModuleSpawnData::Npc {
                spawn_description_property_name,
                ..
            } => Some(spawn_description_property_name),
            ModuleSpawnData::Authored => None,
        }
    }

    /// Borrows additional authored fields in source order.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }

    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let debug_name = fields.borrowed_string("debugName", notes)?;
        let spawn_immediately = fields.boolean("spawnImmediately", notes)?;
        let (data, projected) = if fields.source_type()? == "ModuleSpawnedEntities_NPC" {
            let selector = fields.borrowed_string("spawnDescriptionPropertyName", notes)?;
            let projected = Some(("spawnDescriptionPropertyName", selector.state()));
            (
                ModuleSpawnData::Npc {
                    spawn_description_property_name: selector,
                },
                projected,
            )
        } else {
            fields.note_retained_variant(notes)?;
            (ModuleSpawnData::Authored, None)
        };
        let shared = [
            ("debugName", debug_name.state()),
            ("spawnImmediately", spawn_immediately.state()),
        ];
        Ok(Self {
            source_type: fields.source_type_value()?,
            debug_name,
            spawn_immediately,
            data,
            authored_fields: fields.authored_members_except(shared.into_iter().chain(projected))?,
        })
    }
}

/// An authored module variable and its original binding.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ModuleVariable<'source> {
    source_type: SourcedValue<&'source str>,
    subsumption_variable_name: SourcedValue<&'source str>,
    #[serde(flatten)]
    data: ModuleVariableData<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum ModuleVariableData<'source> {
    LocationEntities {
        location_entities: OwnedSlots<WeakLink<'source>>,
    },
    SpawnedEntities {
        spawned_entities: OwnedSlots<WeakLink<'source>>,
    },
    Authored,
}

impl<'source> ModuleVariable<'source> {
    /// Borrows the original concrete variable type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the authored variable name.
    #[must_use]
    pub const fn subsumption_variable_name(&self) -> &SourcedValue<&'source str> {
        &self.subsumption_variable_name
    }

    /// Borrows the ordered weak location-entity slots.
    #[must_use]
    pub const fn location_entities(
        &self,
    ) -> Option<&SourcedValue<Vec<SourcedValue<Box<SourceWeakReference<'source>>>>>> {
        match &self.data {
            ModuleVariableData::LocationEntities {
                location_entities, ..
            } => Some(location_entities),
            _ => None,
        }
    }

    /// Borrows the ordered weak spawned-entity slots.
    #[must_use]
    pub const fn spawned_entities(
        &self,
    ) -> Option<&SourcedValue<Vec<SourcedValue<Box<SourceWeakReference<'source>>>>>> {
        match &self.data {
            ModuleVariableData::SpawnedEntities {
                spawned_entities, ..
            } => Some(spawned_entities),
            _ => None,
        }
    }

    /// Borrows additional authored fields in source order.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }

    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let subsumption_variable_name = fields.borrowed_string("subsumptionVariableName", notes)?;
        let (data, projected) = match fields.source_type()? {
            "VariableDeclaration_LocationEntities" => {
                let targets = fields.weak_link_array_to(
                    "locationEntities",
                    "ModuleLocationEntities",
                    notes,
                )?;
                let projected = Some(("locationEntities", targets.state()));
                (
                    ModuleVariableData::LocationEntities {
                        location_entities: targets,
                    },
                    projected,
                )
            }
            "VariableDeclaration_SpawnedEntities" => {
                let targets =
                    fields.weak_link_array_to("spawnedEntities", "ModuleSpawnedEntities", notes)?;
                let projected = Some(("spawnedEntities", targets.state()));
                (
                    ModuleVariableData::SpawnedEntities {
                        spawned_entities: targets,
                    },
                    projected,
                )
            }
            _ => {
                fields.note_retained_variant(notes)?;
                (ModuleVariableData::Authored, None)
            }
        };
        let shared = [("subsumptionVariableName", subsumption_variable_name.state())];
        Ok(Self {
            source_type: fields.source_type_value()?,
            subsumption_variable_name,
            data,
            authored_fields: fields.authored_members_except(shared.into_iter().chain(projected))?,
        })
    }
}
