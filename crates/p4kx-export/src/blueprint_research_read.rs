// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUseRole, SourceValue, SourcedValue, StructValue, Value};

use crate::CatalogError;
use crate::blueprint_field_source::{BlueprintField, value_ref};
use crate::blueprint_research_contract::{
    BlueprintResearch, BlueprintResearchEnvelope, BlueprintResearchPointer,
};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::crafting_research::{self, COSTS_BASE, RESEARCH_TYPE, ResearchValue, UNLOCK_BASE};
use crate::source_fields::{FieldCapture, FieldError, FieldViolation};

const RESEARCH: &str = "research";
const COSTS: &str = "researchCosts";
const UNLOCK: &str = "unlockRequirements";

pub(super) fn read<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
    tier: &StructValue<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<BlueprintResearch, CatalogError> {
    let original = BlueprintField::read(fields, tier, RESEARCH)?;
    let envelope = match original
        .fact()
        .map(crate::catalog_read::original::OriginalValueFact::value)
    {
        None => SourceValue::Absent,
        Some(Value::Null) => SourceValue::Null,
        Some(Value::Error(_)) => SourceValue::Unreadable,
        Some(Value::Struct(source)) if source.type_name() == RESEARCH_TYPE => {
            SourceValue::Present(BlueprintResearchEnvelope {
                research_costs: pointer(
                    input,
                    fields,
                    capture,
                    source,
                    (COSTS, COSTS_BASE),
                    notes,
                )?,
                unlock_requirements: pointer(
                    input,
                    fields,
                    capture,
                    source,
                    (UNLOCK, UNLOCK_BASE),
                    notes,
                )?,
            })
        }
        Some(_) => {
            unsupported(fields, capture, tier, &original, notes)?;
            SourceValue::Unsupported
        }
    };
    Ok(BlueprintResearch {
        source: original.reference(),
        envelope: original.selected(envelope)?,
    })
}

fn pointer<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
    source: &StructValue<'source>,
    (name, expected): (&str, &str),
    notes: &mut Vec<FieldError>,
) -> Result<BlueprintResearchPointer, CatalogError> {
    let original = BlueprintField::read(fields, source, name)?;
    let (declared_base, state) = if let Some(fact) = original.fact() {
        let base = declared_base(input, fields, fact)?;
        let declared = base.value().map(String::as_str);
        let state = match crafting_research::member_value(fact.value(), expected, declared) {
            ResearchValue::Unreadable(_) => SourceValue::Unreadable,
            ResearchValue::Null => SourceValue::Null,
            ResearchValue::Present => SourceValue::Present(value_ref(fact)),
            _ => {
                unsupported(fields, capture, source, &original, notes)?;
                SourceValue::Unsupported
            }
        };
        (base, state)
    } else {
        (original.absent()?, SourceValue::Absent)
    };
    Ok(BlueprintResearchPointer {
        source: original.reference(),
        declared_base,
        value: original.selected(state)?,
    })
}

fn declared_base(
    input: &OriginalCatalogInput<'_, '_>,
    fields: &OriginalFlowFields<'_, '_, '_>,
    fact: &crate::catalog_read::original::OriginalValueFact<'_, '_, '_>,
) -> Result<SourcedValue<String>, CatalogError> {
    fact.check_input(input)?;
    let storage = fact.field().storage();
    let storage_inputs = [
        fact.observe(OriginalValuePart::DataType, SourceUseRole::Selector)?,
        fact.observe(OriginalValuePart::IsArray, SourceUseRole::Selector)?,
    ];
    if storage.data_type() != DataType::StrongPointer || storage.is_array() {
        return Ok(fields
            .sources()
            .value(SourceValue::Unsupported, &storage_inputs, true));
    }
    let (name, use_) = input.declared_struct_name(storage.target(), SourceUseRole::Payload)?;
    let mut inputs = storage_inputs.to_vec();
    inputs.push(fact.observe(OriginalValuePart::Target, SourceUseRole::Selector)?);
    inputs.push(use_);
    let name = name.map_or(SourceValue::Unreadable, |name| {
        SourceValue::Present(name.to_owned())
    });
    let complete = matches!(name, SourceValue::Present(_));
    Ok(fields.sources().value(name, &inputs, complete))
}

fn unsupported<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
    parent: &StructValue<'source>,
    original: &BlueprintField<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<(), CatalogError> {
    let Some(fact) = original.fact() else {
        return Ok(());
    };
    let path = fields
        .owner_of(parent)?
        .map(|owner| fields.logical_path(owner))
        .transpose()?
        .unwrap_or_default();
    let violation = FieldViolation::unsupported(parent, &path, fact.field());
    let body = capture.field_failure(&violation)?;
    notes.push(violation.into_error().with_body(body));
    Ok(())
}
