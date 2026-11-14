// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DataForge, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::catalog_json_number::CatalogJsonNumber;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_definition::{
    self, DefinitionFamily, DefinitionIssue, LooseDefinition,
};
use crate::catalog_read::loose_record_link::{self, LooseRecordLink};
use crate::catalog_read::original::{EntryCapture, PhysicalEntryIdentity};
use crate::catalog_table::CatalogTable;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions, Kind};

const FAMILY: DefinitionFamily = DefinitionFamily::new(
    Kind::Creatures,
    "data/libs/planets/creatures/",
    "creature",
    "PlanetCreature",
    "guid",
);
const ENTITY: &str = "EntityClassDefinition";

pub(crate) type PlanetCreatureTable = CatalogTable<PlanetCreature>;

/// Original `PlanetTech` fauna configuration from a physical loose JSON Entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PlanetCreature {
    identity: PhysicalEntryIdentity,
    guid: SourcedValue<CatalogGuid>,
    guid_raw: SourcedValue<String>,
    entity_class: SourcedValue<CatalogGuid>,
    entity_class_raw: SourcedValue<String>,
    entity: LooseRecordLink,
    need_vertical_offset: SourcedValue<bool>,
    slope_tolerance: SourcedValue<CatalogJsonNumber>,
    issues: Vec<DefinitionIssue>,
}

impl LooseDefinition for PlanetCreature {
    fn guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.guid
    }

    fn issues(&mut self) -> &mut Vec<DefinitionIssue> {
        &mut self.issues
    }
}

pub(crate) fn read(
    archive: &Archive,
    forge: &DataForge<'_>,
    original: &OriginalCatalogInput<'_, '_>,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
) -> Result<PlanetCreatureTable, CatalogError> {
    loose_definition::read(
        archive,
        capture,
        ledger,
        options,
        FAMILY,
        |identity, object| project(identity, object, forge, original, options),
    )
}

fn project(
    identity: PhysicalEntryIdentity,
    object: &EntryJsonObject<'_, '_>,
    forge: &DataForge<'_>,
    original: &OriginalCatalogInput<'_, '_>,
    options: &CatalogOptions<'_>,
) -> Result<PlanetCreature, CatalogError> {
    let guid = object.field("guid", entry_json_object::guid)?;
    let guid_raw = object.field("guid", entry_json_object::text)?;
    let entity_class = object.field("entityClass", entry_json_object::guid)?;
    let entity_class_raw = object.field("entityClass", entry_json_object::text)?;
    let need_vertical_offset = object.field("needVerticalOffset", entry_json_object::boolean)?;
    let slope_tolerance = object.field("slopeTolerance", entry_json_object::number)?;
    let entity = loose_record_link::by_guid(
        forge,
        original,
        options,
        &entity_class,
        &entity_class_raw,
        ENTITY,
    )?;
    let mut issues: Vec<_> = loose_definition::identity_issue("guid", &guid)
        .into_iter()
        .collect();
    for (field, state) in [
        ("needVerticalOffset", need_vertical_offset.state()),
        ("slopeTolerance", slope_tolerance.state()),
    ] {
        if state == SourceValueState::Invalid {
            issues.push(DefinitionIssue {
                field,
                code: ExportDiagnosticCode::MalformedSource,
            });
        }
    }
    if let Some(code) = entity
        .state
        .value()
        .copied()
        .and_then(loose_definition::reference_issue)
    {
        issues.push(DefinitionIssue {
            field: "entityClass",
            code,
        });
    }
    Ok(PlanetCreature {
        identity,
        guid,
        guid_raw,
        entity_class,
        entity_class_raw,
        entity,
        need_vertical_offset,
        slope_tolerance,
        issues,
    })
}

#[cfg(test)]
#[path = "planet_creature_tests.rs"]
mod tests;
