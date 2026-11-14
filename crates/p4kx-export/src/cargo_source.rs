// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub(crate) const BASE: &str = "SBaseCargoUnit";
pub(crate) const STANDARD: &str = "SStandardCargoUnit";
pub(crate) const CENTI: &str = "SCentiCargoUnit";
pub(crate) const MICRO: &str = "SMicroCargoUnit";
pub(crate) const OPEN: &str = "InventoryOpenContainerType";
pub(crate) const CLOSED: &str = "InventoryClosedContainerType";

/// A concrete authored cargo amount unit.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CargoUnit {
    Standard,
    Centi,
    Micro,
}

impl CargoUnit {
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
