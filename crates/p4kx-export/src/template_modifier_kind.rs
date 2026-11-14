// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

/// The existing modifier interpretation while its original payload remains source-owned.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum TemplateModifierKind {
    SecurityClearance,
    RequestRentShip,
    ShipKioskFilter,
    FactionHostility,
    HostileMission,
    IgnoreMissionPlayerCriminality,
    LawLicense,
}

impl TemplateModifierKind {
    pub(crate) fn read(name: &str) -> Option<Self> {
        Some(
            match name.strip_prefix("MissionModifier_").unwrap_or(name) {
                "SecurityClearance" => Self::SecurityClearance,
                "RequestRentShip" => Self::RequestRentShip,
                "ShipKioskFilter" => Self::ShipKioskFilter,
                "FactionHostility" => Self::FactionHostility,
                "HostileMission" => Self::HostileMission,
                "IgnoreMissionPlayerCriminality" => Self::IgnoreMissionPlayerCriminality,
                "LawLicense" => Self::LawLicense,
                _ => return None,
            },
        )
    }
}
