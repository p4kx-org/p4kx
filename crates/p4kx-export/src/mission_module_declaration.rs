// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::prepared_definition::PreparedDefinition;
use super::module_property::ModuleMissionProperty;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "mission_module_declaration_content.rs"]
mod content;
#[path = "mission_module_declaration_locations.rs"]
mod locations;

use content::{ModuleSpawnedEntities, ModuleType, ModuleVariable};
use locations::ModuleLocationEntities;

pub(crate) const SOURCE_TYPE: &str = "ModuleDeclaration";
const MISSION_PROPERTY: &str = "MissionProperty";

pub(super) type OwnedSlots<T> = SourcedValue<Vec<SourcedValue<T>>>;
pub(crate) type PreparedModuleDeclaration<'source> =
    PreparedDefinition<'source, MissionModuleDeclaration<'source>>;

/// The physical declaration of a mission module and its owned configuration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionModuleDeclaration<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    #[serde(flatten)]
    configuration: ModuleConfiguration<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ModuleConfiguration<'source> {
    module: SourcedValue<&'source str>,
    module_type: SourcedValue<ModuleType<'source>>,
    location_entities: OwnedSlots<ModuleLocationEntities<'source>>,
    properties: OwnedSlots<ModuleMissionProperty<'source>>,
    spawned_entities: OwnedSlots<ModuleSpawnedEntities<'source>>,
    declared_variables: OwnedSlots<ModuleVariable<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> ModuleConfiguration<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let module = fields.borrowed_string("module", notes)?;
        let module_type = fields.strong_pointer_family_to(
            "moduleType",
            "ModuleDeclarationType_Base",
            notes,
            ModuleType::read,
        )?;
        let location_entities = fields.strong_pointer_family_array_to(
            "locationEntities",
            "ModuleLocationEntities",
            notes,
            ModuleLocationEntities::read,
        )?;
        let properties = fields.class_array_to(
            "properties",
            MISSION_PROPERTY,
            notes,
            ModuleMissionProperty::read,
        )?;
        let spawned_entities = fields.strong_pointer_family_array_to(
            "spawnedEntities",
            "ModuleSpawnedEntities",
            notes,
            ModuleSpawnedEntities::read,
        )?;
        let declared_variables = fields.strong_pointer_family_array_to(
            "declaredVariables",
            "VariableDeclaration_Base",
            notes,
            ModuleVariable::read,
        )?;
        let authored_fields = fields.authored_members_except([
            ("module", module.state()),
            ("moduleType", module_type.state()),
            ("locationEntities", location_entities.state()),
            ("properties", properties.state()),
            ("spawnedEntities", spawned_entities.state()),
            ("declaredVariables", declared_variables.state()),
        ])?;
        Ok(Self {
            module,
            module_type,
            location_entities,
            properties,
            spawned_entities,
            declared_variables,
            authored_fields,
        })
    }
}

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(PreparedModuleDeclaration<'source>, Vec<FieldError>), CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    PreparedDefinition::read(
        tree,
        projection,
        admission,
        cancel,
        |source, fields, _, notes| {
            Ok(MissionModuleDeclaration {
                source,
                configuration: ModuleConfiguration::read(fields, notes)?,
            })
        },
    )
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<PreparedModuleDeclaration<'source>>,
) -> CatalogTable<MissionModuleDeclaration<'source>> {
    PreparedDefinition::order(&mut rows);
    PreparedDefinition::table(forge, SOURCE_TYPE, rows)
}

pub(crate) fn sources<'rows, 'source>(
    rows: &'rows [PreparedModuleDeclaration<'source>],
) -> impl Iterator<Item = (Kind, Guid, &'source str, &'rows [CapturedNumber])> {
    PreparedDefinition::sources(Kind::MissionModuleDeclarations, rows)
}

#[cfg(test)]
#[path = "mission_module_declaration_tests.rs"]
mod tests;
