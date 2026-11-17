// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::value_ref;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::mission_property_set::MissionPropertyLevel;
use crate::record_read::ReadError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldCapture;
use crate::source_value_catalog::SourceValueRef;
use crate::template_value_meaning::{
    TemplateDatasetCondition, TemplateMissionItemSelection, dataset_conditions,
};

/// A physical Generator item's authored inputs within the original Generator source.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct GeneratorMissionItemProperty {
    level: MissionPropertyLevel,
    property_set: SourceValueRef,
    property: SourceValueRef,
    source: SourceValueRef,
    mission_item: TemplateMissionItemSelection,
    conditions: Vec<TemplateDatasetCondition>,
}

pub(crate) fn read<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<Vec<GeneratorMissionItemProperty>, ReadError> {
    let mut items = Vec::new();
    crate::mission_property_sources::visit(fields.root(), capture, |level, params| {
        if let Some(params) = params {
            collect(fields, level, params, &mut items)?;
        }
        Ok(())
    })?;
    Ok(items)
}

fn collect<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    level: MissionPropertyLevel,
    params: &StructValue<'source>,
    items: &mut Vec<GeneratorMissionItemProperty>,
) -> Result<(), ReadError> {
    let owner = fields.owner_of(params)?;
    let Some((_, declaration)) = fields.field(owner, "propertyOverrides")? else {
        return Ok(());
    };
    let physical = |fact: &OriginalValueFact<'_, '_, '_>| Ok(value_ref(fact));
    for property in fields.structs(declaration)? {
        let Some(source) = fields.selected_struct(property, "value")? else {
            continue;
        };
        if !matches!(source.value(), Value::Struct(value) if value.type_name() == "MissionPropertyValue_MissionItem")
        {
            continue;
        }
        items.push(GeneratorMissionItemProperty {
            level,
            property_set: value_ref(declaration),
            property: value_ref(property),
            source: value_ref(source),
            mission_item: TemplateMissionItemSelection::read(fields, source, physical)?,
            conditions: dataset_conditions(fields, source, true, physical)?,
        });
    }
    Ok(())
}

impl GeneratorMissionItemProperty {
    /// The authored handler, contract or sub-contract level.
    #[must_use]
    pub const fn level(&self) -> MissionPropertyLevel {
        self.level
    }

    /// The owning property-set occurrence.
    #[must_use]
    pub const fn property_set(&self) -> SourceValueRef {
        self.property_set
    }

    /// The original property declaration occurrence.
    #[must_use]
    pub const fn property(&self) -> SourceValueRef {
        self.property
    }

    /// The original property value occurrence.
    #[must_use]
    pub const fn source(&self) -> SourceValueRef {
        self.source
    }

    /// The authored mission-item selection bindings.
    #[must_use]
    pub const fn mission_item(&self) -> &TemplateMissionItemSelection {
        &self.mission_item
    }

    /// The original dataset-condition interpretations.
    #[must_use]
    pub fn conditions(&self) -> &[TemplateDatasetCondition] {
        &self.conditions
    }
}
