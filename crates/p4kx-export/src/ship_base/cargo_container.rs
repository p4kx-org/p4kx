// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use p4kx_extraction::{StructValue, Value};

use crate::catalog_v3::cargo_source::{self, CargoUnit};
use crate::record_fields::number_value;

pub(super) enum ContainerInput<'read, 'source> {
    Open([Option<&'read Value<'source>>; 3]),
    Closed {
        unit: CargoUnit,
        amount: Option<&'read Value<'source>>,
    },
    Unavailable(CapacityFailure<'source>),
}

pub(super) enum MeasuredCapacity<'source> {
    Open { dimensions: [f64; 3], scu: f64 },
    Closed { scu: f64, unit: CargoUnit },
    Unavailable(CapacityFailure<'source>),
}

pub(super) type OperandObserver<'observe, 'read, 'source> = dyn FnMut(&'read StructValue<'source>, &'static str, Option<&'read Value<'source>>, OperandRole)
    + 'observe;

#[derive(Clone, Copy)]
pub(super) enum OperandRole {
    Kind,
    Field,
    Amount,
}

impl<'read, 'source> ContainerInput<'read, 'source> {
    pub(super) fn select(
        root: &'read StructValue<'source>,
        observe: &mut OperandObserver<'_, 'read, 'source>,
    ) -> Self {
        let Some(inventory) = structure(operand(root, "inventoryType", OperandRole::Kind, observe))
        else {
            return Self::Unavailable(CapacityFailure::MissingInventoryType);
        };
        match inventory.type_name() {
            cargo_source::OPEN => {
                let dimensions = structure(operand(
                    root,
                    "interiorDimensions",
                    OperandRole::Field,
                    observe,
                ));
                Self::Open(["x", "y", "z"].map(|axis| {
                    dimensions.and_then(|size| operand(size, axis, OperandRole::Amount, observe))
                }))
            }
            cargo_source::CLOSED => Self::select_closed(inventory, observe),
            other => Self::Unavailable(CapacityFailure::UnknownInventoryType(other)),
        }
    }

    fn select_closed(
        inventory: &'read StructValue<'source>,
        observe: &mut OperandObserver<'_, 'read, 'source>,
    ) -> Self {
        let Some(capacity) = structure(operand(inventory, "capacity", OperandRole::Kind, observe))
        else {
            return Self::Unavailable(CapacityFailure::MissingCargoUnit);
        };
        let Some(unit) = CargoUnit::from_source(capacity.type_name()) else {
            return Self::Unavailable(CapacityFailure::MissingCargoUnit);
        };
        Self::Closed {
            unit,
            amount: operand(capacity, unit.amount_field(), OperandRole::Amount, observe),
        }
    }

    pub(super) fn measure(self) -> MeasuredCapacity<'source> {
        match self {
            Self::Open(axes) => measure_open(axes),
            Self::Closed { unit, amount } => measure_closed(unit, amount),
            Self::Unavailable(reason) => MeasuredCapacity::Unavailable(reason),
        }
    }
}

fn operand<'read, 'source>(
    parent: &'read StructValue<'source>,
    name: &'static str,
    role: OperandRole,
    observe: &mut OperandObserver<'_, 'read, 'source>,
) -> Option<&'read Value<'source>> {
    let value = parent.field(name);
    observe(parent, name, value, role);
    value
}

const fn structure<'read, 'source>(
    value: Option<&'read Value<'source>>,
) -> Option<&'read StructValue<'source>> {
    match value {
        Some(Value::Struct(value)) => Some(value),
        _ => None,
    }
}

fn measure_open<'source>(axes: [Option<&Value<'_>>; 3]) -> MeasuredCapacity<'source> {
    let [Some(x), Some(y), Some(z)] = axes.map(|axis| axis.and_then(number_value).map(f64::from))
    else {
        return MeasuredCapacity::Unavailable(CapacityFailure::MissingDimensions);
    };
    let dimensions = [x, y, z];
    cargo_source::open_capacity(dimensions).map_or(
        MeasuredCapacity::Unavailable(CapacityFailure::InvalidDimensions(dimensions)),
        |scu| MeasuredCapacity::Open { dimensions, scu },
    )
}

fn measure_closed<'source>(
    unit: CargoUnit,
    amount: Option<&Value<'_>>,
) -> MeasuredCapacity<'source> {
    let amount = match amount {
        Some(Value::Int32(value)) => Some(f64::from(*value)),
        Some(Value::UInt32(value)) => Some(f64::from(*value)),
        Some(value) => number_value(value).map(f64::from),
        None => None,
    };
    match amount.map(|value| unit.normalize(value)) {
        Some(scu) if cargo_source::is_valid_closed_capacity(scu) => {
            MeasuredCapacity::Closed { scu, unit }
        }
        Some(scu) => MeasuredCapacity::Unavailable(CapacityFailure::InvalidAmount { unit, scu }),
        None => MeasuredCapacity::Unavailable(CapacityFailure::MissingAmount(unit)),
    }
}

pub(super) enum CapacityFailure<'source> {
    MissingInventoryType,
    UnknownInventoryType(&'source str),
    MissingDimensions,
    InvalidDimensions([f64; 3]),
    MissingCargoUnit,
    MissingAmount(CargoUnit),
    InvalidAmount { unit: CargoUnit, scu: f64 },
}

impl fmt::Display for CapacityFailure<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInventoryType => {
                formatter.write_str("the container has no inventory type")
            }
            Self::UnknownInventoryType(name) => {
                write!(formatter, "the container type {name} states no capacity")
            }
            Self::MissingDimensions => {
                formatter.write_str("the container states no interior dimensions")
            }
            Self::InvalidDimensions(dimensions) => write!(
                formatter,
                "the interior dimensions {dimensions:?} must be finite, positive lengths"
            ),
            Self::MissingCargoUnit => {
                formatter.write_str("the container states no capacity in cargo units")
            }
            Self::MissingAmount(unit) => write!(
                formatter,
                "the capacity {} states no {}",
                unit.source_type(),
                unit.amount_field()
            ),
            Self::InvalidAmount { unit, scu } => write!(
                formatter,
                "the capacity {scu} SCU of {} must be finite and nonnegative",
                unit.source_type()
            ),
        }
    }
}
