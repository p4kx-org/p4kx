// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::prepared_definition::PreparedDefinition;
use crate::blueprint_field_source::BlueprintField;
use crate::blueprint_rule_inputs::BlueprintRules;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_row::CatalogRow;
use crate::catalog_table::CatalogTable;
use crate::crafting_records::DISMANTLE_PROCESS;
use crate::crafting_types::DismantleProcess;
use crate::record_observation::RecordObservation;
use crate::record_read::RecordProjection;
use crate::resource_integer::ResourceInt64;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::{CatalogError, Kind};

/// A physical Blueprint Record defining a generic dismantle process.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct DismantleProcessDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    blueprint: SourcedValue<DismantleBlueprint<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

/// The owned blueprint containing a dismantle configuration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct DismantleBlueprint<'source> {
    source_type: SourcedValue<&'source str>,
    process_specific_data: SourcedValue<DismantleConfiguration<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

/// The authored settings for a generic dismantle process.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct DismantleConfiguration<'source> {
    source_type: SourcedValue<&'source str>,
    efficiency: SourcedValue<ResourceNumber>,
    dismantle_time: SourcedValue<DismantleTime<'source>>,
    time_seconds: SourcedValue<ResourceInt64>,
    authored_fields: SourceArenaOutput<'source>,
}

/// The partitioned duration with exact original numeric cells.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct DismantleTime<'source> {
    source_type: SourcedValue<&'source str>,
    days: SourcedValue<ResourceNumber>,
    hours: SourcedValue<ResourceNumber>,
    minutes: SourcedValue<ResourceNumber>,
    seconds: SourcedValue<ResourceNumber>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> DismantleProcessDefinition<'source> {
    /// The physical Record header and the origins of its fields.
    #[must_use]
    pub const fn header(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// The owned blueprint configuration.
    #[must_use]
    pub const fn blueprint(&self) -> &SourcedValue<DismantleBlueprint<'source>> {
        &self.blueprint
    }

    /// Other authored members with their original provenance.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }
}

impl<'source> DismantleBlueprint<'source> {
    /// The original concrete structure type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// The original process configuration.
    #[must_use]
    pub const fn process_specific_data(&self) -> &SourcedValue<DismantleConfiguration<'source>> {
        &self.process_specific_data
    }

    /// Other authored members with their original provenance.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }
}

impl<'source> DismantleConfiguration<'source> {
    /// The original concrete structure type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// The stored dismantle efficiency in its original encoding.
    #[must_use]
    pub const fn efficiency(&self) -> &SourcedValue<ResourceNumber> {
        &self.efficiency
    }

    /// The original duration components.
    #[must_use]
    pub const fn dismantle_time(&self) -> &SourcedValue<DismantleTime<'source>> {
        &self.dismantle_time
    }

    /// The compatible whole-second total and its conversion inputs.
    #[must_use]
    pub const fn time_seconds(&self) -> &SourcedValue<ResourceInt64> {
        &self.time_seconds
    }

    /// Other authored members with their original provenance.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }
}

impl<'source> DismantleTime<'source> {
    /// The original concrete structure type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// The original days component.
    #[must_use]
    pub const fn days(&self) -> &SourcedValue<ResourceNumber> {
        &self.days
    }

    /// The original hours component.
    #[must_use]
    pub const fn hours(&self) -> &SourcedValue<ResourceNumber> {
        &self.hours
    }

    /// The original minutes component.
    #[must_use]
    pub const fn minutes(&self) -> &SourcedValue<ResourceNumber> {
        &self.minutes
    }

    /// The original seconds component.
    #[must_use]
    pub const fn seconds(&self) -> &SourcedValue<ResourceNumber> {
        &self.seconds
    }

    /// Other authored members with their original provenance.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }
}

pub(crate) type PreparedDismantleProcess<'source> =
    PreparedDefinition<'source, DismantleProcessDefinition<'source>>;

pub(crate) struct DismantleRead<'source> {
    pub(crate) rules: BlueprintRules,
    pub(crate) definition: Option<PreparedDismantleProcess<'source>>,
    pub(crate) notes: Vec<FieldError>,
}

/// Reads process-choice evidence and the selected row from the same original Record.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    selected: Option<&DismantleProcess<'source>>,
    cancel: Option<&CancelToken>,
) -> Result<DismantleRead<'source>, CatalogError> {
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let rules = BlueprintRules::dismantle_fields(tree, admission, input, &originals)?;
    let Some(selected) = selected else {
        return Ok(DismantleRead {
            rules,
            definition: None,
            notes: Vec::new(),
        });
    };
    let projected = project(tree, input, admission, &originals, selected.time_seconds)?;
    Ok(DismantleRead {
        rules,
        definition: Some(projected.value),
        notes: projected.notes,
    })
}

pub(crate) fn read_selected<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<Option<RecordProjection<PreparedDismantleProcess<'source>>>, CatalogError> {
    let Some(process) = crate::crafting_records::dismantle(tree) else {
        return Ok(None);
    };
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    project(tree, input, admission, &originals, process.time_seconds).map(Some)
}

fn project<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    originals: &OriginalFlowFields<'_, '_, 'source>,
    converted_seconds: Option<i64>,
) -> Result<RecordProjection<PreparedDismantleProcess<'source>>, CatalogError> {
    let time_seconds = BlueprintField::read(originals, tree.root(), "blueprint")?
        .child("processSpecificData")?
        .child("dismantleTime")?
        .duration_seconds(converted_seconds)?;
    let (value, notes) = PreparedDefinition::read_with_fields(
        tree,
        input,
        admission,
        originals,
        |source, fields, notes| read_definition(source, fields, time_seconds, notes),
    )?;
    Ok(RecordProjection { value, notes })
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut processes: Vec<PreparedDismantleProcess<'source>>,
) -> CatalogTable<DismantleProcessDefinition<'source>> {
    PreparedDefinition::order(&mut processes);
    PreparedDefinition::table(forge, DISMANTLE_PROCESS, processes)
}

pub(crate) fn catalog_rows<'rows>(
    rows: &'rows [PreparedDismantleProcess<'_>],
) -> impl Iterator<Item = CatalogRow> + 'rows {
    PreparedDefinition::catalog_rows(rows, Kind::DismantleProcesses)
}

pub(crate) fn sources<'rows, 'source>(
    rows: &'rows [PreparedDismantleProcess<'source>],
) -> impl Iterator<Item = (Kind, Guid, &'source str, &'rows [CapturedNumber])> {
    PreparedDefinition::sources(Kind::DismantleProcesses, rows)
}

fn read_definition<'source>(
    source: CanonicalRecordHeader<'source>,
    fields: Observed<'_, '_, '_, 'source>,
    time_seconds: SourcedValue<ResourceInt64>,
    notes: &mut Vec<FieldError>,
) -> Result<DismantleProcessDefinition<'source>, CatalogError> {
    let blueprint = fields.declared_owned("blueprint", notes, |fields, notes| {
        let process_specific_data =
            fields.declared_owned("processSpecificData", notes, |fields, notes| {
                read_process(fields, time_seconds, notes)
            })?;
        let authored_fields = fields
            .authored_members_except([("processSpecificData", process_specific_data.state())])?;
        Ok(DismantleBlueprint {
            source_type: fields.source_type_value()?,
            process_specific_data,
            authored_fields,
        })
    })?;
    let authored_fields = fields.authored_members_except([("blueprint", blueprint.state())])?;
    Ok(DismantleProcessDefinition {
        source,
        blueprint,
        authored_fields,
    })
}

fn read_process<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    time_seconds: SourcedValue<ResourceInt64>,
    notes: &mut Vec<FieldError>,
) -> Result<DismantleConfiguration<'source>, CatalogError> {
    let efficiency = fields.authored_number("efficiency", notes)?;
    let dismantle_time = fields.declared_owned("dismantleTime", notes, read_time)?;
    let authored_fields = fields.authored_members_except([
        ("efficiency", efficiency.state()),
        ("dismantleTime", dismantle_time.state()),
    ])?;
    Ok(DismantleConfiguration {
        source_type: fields.source_type_value()?,
        efficiency,
        dismantle_time,
        time_seconds,
        authored_fields,
    })
}

fn read_time<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<DismantleTime<'source>, CatalogError> {
    let days = fields.authored_number("days", notes)?;
    let hours = fields.authored_number("hours", notes)?;
    let minutes = fields.authored_number("minutes", notes)?;
    let seconds = fields.authored_number("seconds", notes)?;
    let authored_fields = fields.authored_members_except([
        ("days", days.state()),
        ("hours", hours.state()),
        ("minutes", minutes.state()),
        ("seconds", seconds.state()),
    ])?;
    Ok(DismantleTime {
        source_type: fields.source_type_value()?,
        days,
        hours,
        minutes,
        seconds,
        authored_fields,
    })
}
