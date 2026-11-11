// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

/// The state of an objective token or of the mission, as the mission flow, the comms
/// notifications and the flow actions name it. A value the catalog does not know is `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub(crate) enum ObjectiveState {
    /// The objective is running.
    Active,
    /// The objective succeeded.
    Completed,
    /// The objective failed.
    Failed,
    /// The player abandoned the objective.
    Abandoned,
}

impl ObjectiveState {
    /// Returns the state the stored enum name `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        Some(match stored {
            "Active" => Self::Active,
            "Completed" => Self::Completed,
            "Failed" => Self::Failed,
            "Abandoned" => Self::Abandoned,
            _ => return None,
        })
    }
}

/// The category of an objective display. A value the catalog does not know is `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub(crate) enum ObjectiveCategory {
    /// A primary objective.
    Primary,
}

impl ObjectiveCategory {
    /// Returns the category the stored enum name `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        (stored == "Primary").then_some(Self::Primary)
    }
}

/// When a comms notification is shown relative to its HUD notification. A value the catalog
/// does not know is `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub(crate) enum CommsTiming {
    /// Both at once.
    Simultaneous,
    /// The comms message first.
    CommsNotificationFirst,
    /// The HUD notification first.
    HudNotificationFirst,
}

impl CommsTiming {
    /// Returns the timing the stored enum name `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        Some(match stored {
            "Simultaneous" => Self::Simultaneous,
            "CommsNotificationFirst" => Self::CommsNotificationFirst,
            "HUDNotificationFirst" => Self::HudNotificationFirst,
            _ => return None,
        })
    }
}
