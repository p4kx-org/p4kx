// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    RecordTree, SourceUse, SourceUseRole, SourcedValue, StructValue, Value, ValueError,
};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::commodity_inputs::CommodityInputs;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::resource_group::Groups;

/// Reproduces the first selected group without replacing original scan order by GUID order.
pub(crate) fn first_group(
    origins: &CatalogOrigins,
    subtype: Option<CatalogGuid>,
    links: &[SourceUse],
    groups: &Groups,
) -> Result<SourcedValue<Option<CatalogGuid>>, CatalogError> {
    let mut inputs = CommodityInputs::new(origins);
    inputs.extend(links.iter().cloned(), true);
    let Some(subtype) = subtype else {
        return Ok(inputs.value(None));
    };
    let choice = groups.first_of_resource(&subtype.0);
    for source in choice.selectors() {
        for field in [
            RecordHeaderInput::NormalizedPath,
            RecordHeaderInput::Field(RecordHeaderField::SourceType),
            RecordHeaderInput::Field(RecordHeaderField::RecordName),
        ] {
            inputs.header_input(source.record_index, field, SourceUseRole::Selector)?;
        }
        inputs.at(source.record_index, "resources", source.resources_role)?;
    }
    if let Some(source) = choice.selected {
        inputs.header(
            source.record_index,
            RecordHeaderField::Guid,
            SourceUseRole::Payload,
        )?;
        inputs.at(
            source.record_index,
            &format!("resources[{}]", source.position),
            SourceUseRole::Selector,
        )?;
    }
    inputs.require_complete(choice.complete);
    Ok(inputs.value(choice.group))
}

/// Reproduces the finite source grams-per-cubic-centimetre convenience value.
pub(crate) fn density(
    origins: &CatalogOrigins,
    resource: Option<&RecordTree<'_>>,
    links: &[SourceUse],
    complete: bool,
) -> Result<SourcedValue<Option<f32>>, CatalogError> {
    let mut inputs = CommodityInputs::new(origins);
    inputs.extend(links.iter().cloned(), complete);
    let Some(resource) = resource else {
        return Ok(inputs.value(None));
    };
    let index = resource.record_index();
    let Some(density) = structure(
        resource.root(),
        "densityType",
        "densityType",
        index,
        &mut inputs,
    )?
    else {
        return Ok(inputs.value(None));
    };
    let Some(unit) = structure(
        density,
        "densityUnit",
        "densityType.densityUnit",
        index,
        &mut inputs,
    )?
    else {
        return Ok(inputs.value(None));
    };
    let field = "densityType.densityUnit.gramsPerCubicCentimeter";
    let raw = unit.field("gramsPerCubicCentimeter");
    observe(raw, index, field, &mut inputs)?;
    let value = match raw {
        Some(Value::Single(value)) if value.is_finite() => Some(*value),
        _ => None,
    };
    if value.is_some() {
        inputs.at(index, field, SourceUseRole::Payload)?;
    }
    Ok(inputs.value(value))
}

fn structure<'tree, 'source>(
    parent: &'tree StructValue<'source>,
    name: &str,
    path: &str,
    index: u32,
    inputs: &mut CommodityInputs<'_>,
) -> Result<Option<&'tree StructValue<'source>>, CatalogError> {
    let raw = parent.field(name);
    observe(raw, index, path, inputs)?;
    let Some(Value::Struct(value)) = raw else {
        return Ok(None);
    };
    inputs.at(index, &format!("{path}.$type"), SourceUseRole::Selector)?;
    Ok(Some(value))
}

fn observe(
    raw: Option<&Value<'_>>,
    index: u32,
    field: &str,
    inputs: &mut CommodityInputs<'_>,
) -> Result<(), CatalogError> {
    let unreadable = matches!(raw, Some(Value::Error(_)));
    inputs.at(
        index,
        field,
        if unreadable {
            SourceUseRole::Attempted
        } else {
            SourceUseRole::Selector
        },
    )?;
    inputs.require_complete(!unreadable);
    Ok(())
}

/// Reproduces the resolved source refinement GUID without following the target body.
pub(crate) fn refined(
    origins: &CatalogOrigins,
    resource: Option<&RecordTree<'_>>,
    links: &[SourceUse],
    complete: bool,
) -> Result<SourcedValue<Option<CatalogGuid>>, CatalogError> {
    let mut inputs = CommodityInputs::new(origins);
    inputs.extend(links.iter().cloned(), complete);
    let Some(resource) = resource else {
        return Ok(inputs.value(None));
    };
    let raw = resource.field("refinedVersion");
    let unresolved = matches!(
        raw,
        Some(Value::Error(
            ValueError::DanglingReference { .. } | ValueError::UnexpectedReferenceType { .. }
        ))
    );
    if unresolved {
        inputs.at(
            resource.record_index(),
            "refinedVersion",
            SourceUseRole::Selector,
        )?;
    } else {
        observe(raw, resource.record_index(), "refinedVersion", &mut inputs)?;
    }
    let value = match raw {
        Some(Value::Reference(target)) if target.location().is_none() => {
            inputs.at(
                resource.record_index(),
                "refinedVersion",
                SourceUseRole::Payload,
            )?;
            inputs.header(
                target.record_index(),
                RecordHeaderField::Guid,
                SourceUseRole::Selector,
            )?;
            Some(CatalogGuid(target.guid()))
        }
        _ => None,
    };
    Ok(inputs.value(value))
}
