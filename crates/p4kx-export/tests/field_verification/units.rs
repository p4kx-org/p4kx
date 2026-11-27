// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Unit {
    Dimensionless,
    Seconds,
    RoundsPerMinute,
    Metres,
    Scu,
    MicroScu,
    Resource {
        guid: String,
        representation: ResourceRepresentation,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ResourceRepresentation {
    StandardUnits,
    PowerSegments,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Normalization {
    None,
    MicroScuToScu,
}

pub(crate) fn scale(reference: &Unit, catalog: &Unit, normalization: Normalization) -> Option<f64> {
    match normalization {
        Normalization::None if reference == catalog => Some(1.0),
        Normalization::MicroScuToScu if *reference == Unit::MicroScu && *catalog == Unit::Scu => {
            Some(1e-6)
        }
        _ => None,
    }
}
