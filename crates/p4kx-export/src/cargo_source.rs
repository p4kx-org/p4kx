// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub(crate) const BASE: &str = "SBaseCargoUnit";
pub(crate) const STANDARD: &str = "SStandardCargoUnit";
pub(crate) const CENTI: &str = "SCentiCargoUnit";
pub(crate) const MICRO: &str = "SMicroCargoUnit";
pub(crate) const OPEN: &str = "InventoryOpenContainerType";
pub(crate) const CLOSED: &str = "InventoryClosedContainerType";
const CUBIC_METRES_PER_SCU: f64 = 1.953_125;

/// A concrete authored cargo amount unit.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CargoUnit {
    Standard,
    Centi,
    Micro,
}

impl CargoUnit {
    pub(crate) fn normalize(self, amount: f64) -> f64 {
        let divisor = match self {
            Self::Standard => 1.0,
            Self::Centi => 100.0,
            Self::Micro => 1_000_000.0,
        };
        amount / divisor
    }

    pub(crate) fn from_source(name: &str) -> Option<Self> {
        match name {
            STANDARD => Some(Self::Standard),
            CENTI => Some(Self::Centi),
            MICRO => Some(Self::Micro),
            _ => None,
        }
    }

    pub(crate) const fn amount_field(self) -> &'static str {
        match self {
            Self::Standard => "standardCargoUnits",
            Self::Centi => "centiSCU",
            Self::Micro => "microSCU",
        }
    }

    pub(crate) const fn source_type(self) -> &'static str {
        match self {
            Self::Standard => STANDARD,
            Self::Centi => CENTI,
            Self::Micro => MICRO,
        }
    }
}

pub(crate) fn open_capacity(dimensions: [f64; 3]) -> Option<f64> {
    let [x, y, z] = dimensions;
    dimensions
        .iter()
        .all(|side| side.is_finite() && *side > 0.0)
        .then(|| x * y * z / CUBIC_METRES_PER_SCU)
}

pub(crate) fn is_valid_closed_capacity(amount: f64) -> bool {
    amount.is_finite() && amount >= 0.0
}

pub(crate) fn round_capacity(value: f64) -> f64 {
    let rounded = (value * 1e4).round() / 1e4;
    if rounded.abs() < f64::EPSILON {
        0.0
    } else {
        rounded
    }
}
