// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceValue, SourceValueState, SourcedValue};

use crate::catalog_v3::cargo_source::{self, CargoUnit};
use crate::catalog_v3::catalog_capacity::CargoAmount;
use crate::catalog_v3::inventory_container_definition::PreparedInventoryContainer;
use crate::catalog_v3::inventory_container_type::InventoryContainerType;
use crate::catalog_v3::inventory_dimensions::InventoryVector3;
use crate::resource_float::ResourceFloat32;

pub(super) fn calculate<'rows>(
    container: &'rows PreparedInventoryContainer<'_>,
    origins: &mut Vec<&'rows FieldOrigin>,
) -> SourceValue<f64> {
    fn read<'rows>(
        container: &'rows PreparedInventoryContainer<'_>,
        origins: &mut Vec<&'rows FieldOrigin>,
    ) -> Result<f64, SourceValue<f64>> {
        match value(container.inventory_type(), origins)? {
            InventoryContainerType::Open { .. } => open(container.interior_dimensions(), origins),
            InventoryContainerType::Closed { capacity, .. } => {
                let amount = value(capacity, origins)?
                    .cargo_amount()
                    .ok_or(SourceValue::Unsupported)?;
                closed(amount, origins)
            }
            InventoryContainerType::Base { .. } | InventoryContainerType::UnknownType { .. } => {
                Err(SourceValue::Unsupported)
            }
        }
    }
    match read(container, origins) {
        Ok(scu) => SourceValue::Present(cargo_source::round_capacity(scu)),
        Err(state) => state,
    }
}

fn open<'rows>(
    dimensions: &'rows SourcedValue<InventoryVector3<'_>>,
    origins: &mut Vec<&'rows FieldOrigin>,
) -> Result<f64, SourceValue<f64>> {
    let [x, y, z] = value(dimensions, origins)?.coordinates();
    origins.extend([x.origin(), y.origin(), z.origin()]);
    let dimensions = [finite(x)?, finite(y)?, finite(z)?];
    cargo_source::open_capacity(dimensions).ok_or(SourceValue::Invalid)
}

fn closed<'rows>(
    amount: CargoAmount<'rows>,
    origins: &mut Vec<&'rows FieldOrigin>,
) -> Result<f64, SourceValue<f64>> {
    let scu = match amount {
        CargoAmount::Standard(amount) => {
            origins.push(amount.origin());
            CargoUnit::Standard.normalize(finite(amount)?)
        }
        CargoAmount::Integer(unit, amount) => unit.normalize(f64::from(*value(amount, origins)?)),
    };
    cargo_source::is_valid_closed_capacity(scu)
        .then_some(scu)
        .ok_or(SourceValue::Invalid)
}

fn finite(cell: &SourcedValue<ResourceFloat32>) -> Result<f64, SourceValue<f64>> {
    cell.value()
        .ok_or_else(|| unavailable(cell.state()))?
        .finite
        .map(f64::from)
        .ok_or(SourceValue::Invalid)
}

fn value<'rows, T>(
    cell: &'rows SourcedValue<T>,
    origins: &mut Vec<&'rows FieldOrigin>,
) -> Result<&'rows T, SourceValue<f64>> {
    origins.push(cell.origin());
    cell.value().ok_or_else(|| unavailable(cell.state()))
}

pub(super) const fn unavailable(state: SourceValueState) -> SourceValue<f64> {
    match state {
        SourceValueState::Absent => SourceValue::Absent,
        SourceValueState::Null => SourceValue::Null,
        SourceValueState::Unsupported => SourceValue::Unsupported,
        SourceValueState::Invalid => SourceValue::Invalid,
        _ => SourceValue::Unreadable,
    }
}
