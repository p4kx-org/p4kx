// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue};

use super::super::Observed;
use super::fields;
use super::selection::SelectedSource;
use crate::CatalogError;
use crate::catalog_read::original::OriginalValuePart;
use crate::contract_insurance::{ShipInsurance, ShipInsuranceParams};
use crate::source_fields::FieldError;

pub(super) fn read<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    selected: &SelectedSource<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<Box<ShipInsurance<'source>>>, CatalogError> {
    let Some(value) = selected.value else {
        return Ok(selected.missing(fields.originals.sources()));
    };
    let fact = fields.originals.fact(value)?;
    let fields = fields.at_owner(fact);
    let restore_on_claim = selected.bind(
        fields.originals.sources(),
        fields.boolean("restoreOnClaim", notes)?,
    )?;
    let ship_insurance_params = fields.declared_owned("shipInsuranceParams", notes, params)?;
    let ship_insurance_params = selected.bind(fields.originals.sources(), ship_insurance_params)?;
    let parent = ship_insurance_params.origin().clone();
    let ship_insurance_params = ship_insurance_params
        .try_map(|params| add_parameter_parents(params, fields.originals.sources(), &parent))?;
    let additional_fields = fields.authored_members_except([
        ("restoreOnClaim", restore_on_claim.state()),
        ("shipInsuranceParams", ship_insurance_params.state()),
    ])?;
    let value = Box::new(ShipInsurance {
        static_element_ordinal: selected.ordinal(fields.originals)?,
        restore_on_claim,
        params: ship_insurance_params,
        additional_fields,
    });
    let value = fact
        .bind(
            OriginalValuePart::ConcreteType,
            SourceValue::Present(value),
            SourceUseRole::Selector,
            true,
        )?
        .into_cell(fields.originals.sources());
    selected.bind(fields.originals.sources(), value)
}

fn params<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<ShipInsuranceParams<'source>, CatalogError> {
    let ship_entity_class_name = fields::text(fields, "shipEntityClassName", notes)?;
    let base_wait_time_minutes = fields.authored_number("baseWaitTimeMinutes", notes)?;
    let mandatory_wait_time_minutes = fields.authored_number("mandatoryWaitTimeMinutes", notes)?;
    let expedite_fee = fields.authored_number("baseExpeditingFee", notes)?;
    let additional_fields = fields.authored_members_except([
        ("shipEntityClassName", ship_entity_class_name.state()),
        ("baseWaitTimeMinutes", base_wait_time_minutes.state()),
        (
            "mandatoryWaitTimeMinutes",
            mandatory_wait_time_minutes.state(),
        ),
        ("baseExpeditingFee", expedite_fee.state()),
    ])?;
    Ok(ShipInsuranceParams {
        ship_entity_class_name,
        base_wait_time_minutes,
        mandatory_wait_time_minutes,
        expedite_fee,
        additional_fields,
    })
}

fn add_parameter_parents<'source>(
    mut params: ShipInsuranceParams<'source>,
    uses: &p4kx_extraction::SourceUses,
    parent: &p4kx_extraction::FieldOrigin,
) -> Result<ShipInsuranceParams<'source>, CatalogError> {
    params.ship_entity_class_name =
        uses.add_value_origins(params.ship_entity_class_name, &[parent])?;
    params.base_wait_time_minutes =
        uses.add_value_origins(params.base_wait_time_minutes, &[parent])?;
    params.mandatory_wait_time_minutes =
        uses.add_value_origins(params.mandatory_wait_time_minutes, &[parent])?;
    params.expedite_fee = uses.add_value_origins(params.expedite_fee, &[parent])?;
    Ok(params)
}
