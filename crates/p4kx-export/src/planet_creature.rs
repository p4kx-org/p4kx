// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, SourceUses, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::catalog_json_number::CatalogJsonNumber;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_definition::{
    self, DefinitionFamily, DefinitionIssue, LooseDefinition, Selection,
};
use crate::catalog_read::loose_record_link::{self, GuidRecordLookup, LooseRecordLink};
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
#[non_exhaustive]
pub struct PlanetCreature {
    /// The physical Entry that owns this definition.
    pub identity: PhysicalEntryIdentity,
    /// The parsed creature GUID.
    pub guid: SourcedValue<CatalogGuid>,
    /// The original creature GUID text.
    pub guid_raw: SourcedValue<String>,
    /// The authored Entity class GUID.
    pub entity_class: SourcedValue<CatalogGuid>,
    /// The original Entity class GUID text.
    pub entity_class_raw: SourcedValue<String>,
    /// The linked physical Record header candidates and resolution state.
    pub entity: LooseRecordLink,
    /// The authored vertical offset flag.
    pub need_vertical_offset: SourcedValue<bool>,
    /// The authored slope tolerance with its original numeric spelling.
    pub slope_tolerance: SourcedValue<CatalogJsonNumber>,
    /// Source issues observed in this owner selection.
    pub issues: Vec<DefinitionIssue>,
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
    original: &OriginalCatalogInput<'_, '_>,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
) -> Result<PlanetCreatureTable, CatalogError> {
    read_selected(
        archive,
        capture,
        ledger,
        options,
        Selection::All,
        &mut |guid, selector, expected| {
            loose_record_link::resolve_guid(
                guid,
                selector,
                original,
                expected,
                options.filter,
                options.cancel,
            )
        },
    )
}

pub(crate) fn read_selected(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
    selection: Selection<'_>,
    lookup: &mut GuidRecordLookup<'_>,
) -> Result<PlanetCreatureTable, CatalogError> {
    loose_definition::read_selected(
        archive,
        capture,
        ledger,
        options,
        FAMILY,
        selection,
        |identity, object| project(identity, object, capture.source_uses(), lookup),
    )
}

fn project(
    identity: PhysicalEntryIdentity,
    object: &EntryJsonObject<'_, '_>,
    sources: &SourceUses,
    lookup: &mut GuidRecordLookup<'_>,
) -> Result<PlanetCreature, CatalogError> {
    let guid = object.field("guid", entry_json_object::guid)?;
    let guid_raw = object.field("guid", entry_json_object::text)?;
    let entity_class = object.field("entityClass", entry_json_object::guid)?;
    let entity_class_raw = object.field("entityClass", entry_json_object::text)?;
    let need_vertical_offset = object.field("needVerticalOffset", entry_json_object::boolean)?;
    let slope_tolerance = object.field("slopeTolerance", entry_json_object::number)?;
    let entity = loose_record_link::by_guid_using(
        sources,
        &entity_class,
        &entity_class_raw,
        ENTITY,
        lookup,
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
