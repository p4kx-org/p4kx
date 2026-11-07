// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::Family;

impl Family {
    pub(crate) const AUDIO: [Self; 8] = [
        Self::LuxuryShip,
        Self::Gladius,
        Self::Avenger,
        Self::Hornet,
        Self::Constellation,
        Self::Cyclone,
        Self::Freelancer,
        Self::Cutlass,
    ];
    pub(crate) const STREAMED: [Self; 5] = [
        Self::LuxuryShip,
        Self::Gladius,
        Self::Hornet,
        Self::Constellation,
        Self::Cyclone,
    ];
}
