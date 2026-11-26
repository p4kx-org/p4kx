// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::manufacturer::Manufacturer;
use super::number::Number;
use super::record_identity::{self, RecordIdentity, Variant};
use super::selected_texts::TextRefs;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::contract_insurance::{ShipInsurance, ShipInsuranceParams};
use crate::ship::ShipSource;
use crate::source_value_catalog::SourceRecordReference;
use crate::{MassBasis, Playability, VehicleKind};

const SHIPS: &str = "ships";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Ship<'source> {
    #[serde(flatten)]
    identity: RecordIdentity<'source>,
    name: Cell<&'source PublicNameRef>,
    description: Cell<&'source GameTextRef>,
    role: Cell<&'source PublicNameRef>,
    career: Cell<&'source PublicNameRef>,
    crew_size: Cell<Number<'source>>,
    mass: Cell<&'source f64>,
    mass_basis: Cell<&'source MassBasis>,
    hull_health: Cell<&'source f64>,
    size: Cell<Number<'source>>,
    vehicle_kind: Cell<&'source VehicleKind>,
    playability: Cell<&'source Playability>,
    variant_of: Cell<Variant<'source>>,
    vehicle_manufacturer: Manufacturer<'source>,
    attachment_manufacturer: Manufacturer<'source>,
    insurance: Cell<Insurance<'source>>,
}

impl<'source> Ship<'source> {
    pub(super) fn project(
        row: &'source ShipSource<'source>,
        texts: &mut TextRefs<'source>,
        verify: impl Fn(&SourceRecordReference<'_>) -> Result<(), ReducedError>,
    ) -> Result<Self, ReducedError> {
        let address = row.source();
        let record_index = address
            .record_index()
            .value()
            .copied()
            .ok_or(ReducedError::IncompleteScope(SHIPS))?;
        let variant = row
            .variant_of()
            .ok_or(ReducedError::IncompleteScope(SHIPS))?;
        texts.name(row.name().value());
        texts.game_text(row.description().value());
        texts.name(row.role().value());
        texts.name(row.career().value());
        Ok(Self {
            identity: address.into(),
            name: row.name().into(),
            description: row.description().into(),
            role: row.role().into(),
            career: row.career().into(),
            crew_size: Cell::from(row.crew_size()).map(Number::from),
            mass: row.mass().into(),
            mass_basis: row.mass_basis().into(),
            hull_health: row.hull_health().into(),
            size: Cell::from(row.size()).map(Number::from),
            vehicle_kind: row.vehicle_kind().into(),
            playability: row.playability().into(),
            variant_of: record_identity::variant(variant),
            vehicle_manufacturer: Manufacturer::vehicle(
                row.vehicle_manufacturer(),
                record_index,
                &verify,
            )?,
            attachment_manufacturer: Manufacturer::attachment(
                row.attachment_manufacturer(),
                record_index,
                verify,
            )?,
            insurance: Cell::from(row.insurance()).map(|value| Insurance::from(value.as_ref())),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Insurance<'source> {
    static_element_ordinal: Cell<&'source u32>,
    restore_on_claim: Cell<&'source bool>,
    ship_insurance_params: Cell<InsuranceParams<'source>>,
}

impl<'source> From<&'source ShipInsurance<'source>> for Insurance<'source> {
    fn from(insurance: &'source ShipInsurance<'source>) -> Self {
        Self {
            static_element_ordinal: insurance.static_element_ordinal().into(),
            restore_on_claim: insurance.restore_on_claim().into(),
            ship_insurance_params: Cell::from(insurance.params()).map(InsuranceParams::from),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InsuranceParams<'source> {
    ship_entity_class_name: Cell<&'source str>,
    base_wait_time_minutes: Cell<Number<'source>>,
    mandatory_wait_time_minutes: Cell<Number<'source>>,
    expedite_fee: Cell<Number<'source>>,
}

impl<'source> From<&'source ShipInsuranceParams<'source>> for InsuranceParams<'source> {
    fn from(params: &'source ShipInsuranceParams<'source>) -> Self {
        Self {
            ship_entity_class_name: Cell::from(params.ship_entity_class_name()).map(|value| *value),
            base_wait_time_minutes: Cell::from(params.base_wait_time_minutes()).map(Number::from),
            mandatory_wait_time_minutes: Cell::from(params.mandatory_wait_time_minutes())
                .map(Number::from),
            expedite_fee: Cell::from(params.expedite_fee()).map(Number::from),
        }
    }
}
