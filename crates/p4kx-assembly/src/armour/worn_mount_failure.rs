// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::PortHelperDiagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MountProblem {
    NoSlot,
    MissingRecord,
    MissingHolder,
    MissingPort,
    AmbiguousPort,
    IncompatibleItem,
    UnprovenRequirements,
    NoHelper,
    OccupiedPort,
    SelfMount,
    OccupancyUnproven,
}

impl MountProblem {
    pub(super) const fn message(self) -> &'static str {
        match self {
            Self::NoSlot => "the loadout supplies no compatible free attachment port",
            Self::MissingRecord => "the selected item's original Record is unavailable",
            Self::MissingHolder => "the current holder's original Record is unavailable",
            Self::MissingPort => "the current holder does not declare the original port",
            Self::AmbiguousPort => "the loadout supplies competing original or current ports",
            Self::IncompatibleItem => "the current port's item type, subtype or size rules fail",
            Self::UnprovenRequirements => "the current port or item has unproven tag requirements",
            Self::NoHelper => "the current port does not declare a rigid attachment helper",
            Self::OccupiedPort => "the physical port is occupied or claimed more than once",
            Self::SelfMount => "the selected piece would hold itself",
            Self::OccupancyUnproven => {
                "the original occupancy of a compatible port could not be established"
            }
        }
    }
}

pub(super) struct MountFailure {
    pub(super) problem: MountProblem,
    pub(super) helper: Option<Box<PortHelperDiagnostic>>,
}

impl MountFailure {
    pub(super) fn without_helper(diagnostic: Option<PortHelperDiagnostic>) -> Self {
        Self {
            problem: MountProblem::NoHelper,
            helper: diagnostic.map(Box::new),
        }
    }
}

impl From<MountProblem> for MountFailure {
    fn from(problem: MountProblem) -> Self {
        Self {
            problem,
            helper: None,
        }
    }
}
