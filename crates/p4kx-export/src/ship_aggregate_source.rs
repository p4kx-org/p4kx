// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceUseRole, SourceValue, SourcedValue, Value};

use super::super::Observed;
use super::selection::SelectedSource;
use crate::catalog_read::original::OriginalValuePart;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

/// Original same-read operands of the vehicle choice and physics fallback.
#[derive(Clone)]
pub(crate) struct ShipAggregateOperands {
    pub(crate) record_index: u32,
    pub(crate) vehicle: SourcedValue<bool>,
    pub(crate) modification: SourcedValue<bool>,
    pub(crate) physics: SourcedValue<f32>,
    pub(crate) physics_selector: FieldOrigin,
}

struct PhysicsOperand {
    value: SourcedValue<f32>,
    selector: FieldOrigin,
}

pub(super) fn read<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    vehicle: &SelectedSource<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<(ShipAggregateOperands, Vec<FieldError>), CatalogError> {
    let physics = SelectedSource::read(
        fields.originals,
        "Components",
        "SEntityPhysicsControllerParams",
    )?;
    let mut physics_notes = Vec::new();
    let operand = if let Some(value) = physics.value {
        physics_mass(
            fields.at_owner(fields.originals.fact(value)?),
            &mut physics_notes,
        )?
    } else {
        let value = physics.missing(fields.originals.sources());
        PhysicsOperand {
            selector: value.origin().clone(),
            value,
        }
    };
    let uses = fields.originals.sources();
    let prefix = uses.origin(&physics.prefix.uses, physics.prefix.complete);
    Ok((
        ShipAggregateOperands {
            record_index: fields.originals.record_index(),
            vehicle: super::fields::selected(fields, vehicle, |source| {
                text_choice(source, "vehicleDefinition", notes)
            })?,
            modification: super::fields::selected(fields, vehicle, |source| {
                text_choice(source, "modification", notes)
            })?,
            physics: physics.bind(uses, operand.value)?,
            physics_selector: uses.origin_with_origins([&operand.selector, &prefix], true)?,
        },
        physics_notes,
    ))
}

fn text_choice(
    fields: Observed<'_, '_, '_, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<bool>, CatalogError> {
    let Some((_, fact)) = fields.originals.field(fields.owner, name)? else {
        return fields.absent();
    };
    let state = match fact.value() {
        Value::String(value) | Value::Enum(value) | Value::Locale(value) => {
            SourceValue::Present(!value.trim().is_empty())
        }
        Value::Null => SourceValue::Null,
        value => fields.failure(name, value, "a nullable path text", notes)?,
    };
    fields.bind_field(
        fact,
        OriginalValuePart::Value,
        state,
        SourceUseRole::Selector,
    )
}

fn physics_mass(
    fields: Observed<'_, '_, '_, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<PhysicsOperand, CatalogError> {
    let mut original = None;
    let container = fields.declared_owned("PhysType", notes, |source, notes| {
        original = Some(number(source, notes)?);
        Ok(())
    })?;
    let uses = fields.originals.sources();
    if let Some(value) = original {
        Ok(PhysicsOperand {
            value: uses.add_value_origins(value.value, &[container.origin()])?,
            selector: uses.origin_with_origins([&value.selector, container.origin()], true)?,
        })
    } else {
        let selector = container.origin().clone();
        let value = container.try_map(|()| {
            Err::<f32, CatalogError>(ExportAccountingError::ConflictingSourceDatum.into())
        })?;
        Ok(PhysicsOperand { value, selector })
    }
}

fn number(
    fields: Observed<'_, '_, '_, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<PhysicsOperand, CatalogError> {
    const NAME: &str = "Mass";
    let Some((_, fact)) = fields.originals.field(fields.owner, NAME)? else {
        let value = fields.absent()?;
        return Ok(PhysicsOperand {
            selector: value.origin().clone(),
            value,
        });
    };
    let selector = fields.bind_field(
        fact,
        OriginalValuePart::Value,
        SourceValue::Present(()),
        SourceUseRole::Selector,
    )?;
    let state = match fact.value() {
        Value::Null => SourceValue::Null,
        value => match crate::record_fields::number_value(value) {
            Some(value) => SourceValue::Present(value),
            None => fields.failure(NAME, value, "a finite legacy numeric mass", notes)?,
        },
    };
    let role = if matches!(state, SourceValue::Present(_)) {
        SourceUseRole::Conversion
    } else {
        SourceUseRole::Selector
    };
    Ok(PhysicsOperand {
        value: fields.bind_field(fact, OriginalValuePart::Value, state, role)?,
        selector: selector.origin().clone(),
    })
}
