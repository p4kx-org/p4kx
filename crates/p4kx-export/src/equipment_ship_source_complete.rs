// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};

use crate::catalog_v3::authored_fields::authored_manufacturer::ManufacturerValue;
use crate::contract_insurance::{ShipInsurance, ShipInsuranceParams};
use crate::ship::ShipSource;
use crate::source_value_catalog::SourceRecordReference;

#[path = "equipment_ship_source_arena_complete.rs"]
mod arena;

pub(super) fn complete(row: &ShipSource<'_>) -> bool {
    let header = row.source();
    let guid = header.guid();
    let origins = [
        guid.origin(),
        header.record_name().origin(),
        header.class_name().origin(),
        header.record_path().origin(),
        header.record_index().origin(),
        header.source_type().origin(),
        header.file_name().origin(),
        header.metadata_tag().origin(),
    ];
    origins
        .into_iter()
        .all(p4kx_extraction::FieldOrigin::source_set_complete)
        && known(row.name())
        && known(row.description())
        && known(row.role())
        && known(row.career())
        && known(row.crew_size())
        && known(row.vehicle_definition())
        && known(row.vehicle_kind())
        && known(row.size())
        && known(row.mass())
        && known(row.mass_basis())
        && known(row.hull_health())
        && known(row.vehicle_manufacturer().component_ordinal())
        && manufacturer(row.vehicle_manufacturer().manufacturer())
        && known(row.attachment_manufacturer().component_ordinal())
        && known(row.attachment_manufacturer().attach_def_present())
        && manufacturer(row.attachment_manufacturer().manufacturer())
        && known(row.insurance())
        && row.insurance().value().is_none_or(|value| insurance(value))
        && known(row.playability())
        && known(row.playability_inputs().crew_manifest_component_ordinal())
        && known(row.playability_inputs().crew_manifest())
        && row
            .playability_inputs()
            .crew_manifest()
            .value()
            .is_none_or(|value| reference(value))
}

fn known<T>(cell: &SourcedValue<T>) -> bool {
    matches!(
        cell.state(),
        SourceValueState::Present | SourceValueState::Absent | SourceValueState::Null
    ) && cell.origin().source_set_complete()
}

fn manufacturer(cell: &SourcedValue<ManufacturerValue<'_>>) -> bool {
    known(cell)
        && cell.value().is_none_or(|value| match value {
            ManufacturerValue::Record(value) => reference(value),
            ManufacturerValue::Authored { authored } => arena::complete(authored),
        })
}

fn reference(value: &SourceRecordReference<'_>) -> bool {
    known(&value.guid())
        && known(value.target_record_index())
        && known(value.record_path())
        && known(value.target_type())
}

fn insurance(value: &ShipInsurance<'_>) -> bool {
    known(value.static_element_ordinal())
        && known(value.restore_on_claim())
        && known(value.params())
        && value.params().value().is_none_or(params)
        && arena::complete(value.additional_fields())
}

fn params(value: &ShipInsuranceParams<'_>) -> bool {
    known(value.ship_entity_class_name())
        && known(value.base_wait_time_minutes())
        && known(value.mandatory_wait_time_minutes())
        && known(value.expedite_fee())
        && arena::complete(value.additional_fields())
}
