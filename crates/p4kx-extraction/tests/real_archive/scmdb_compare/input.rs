// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::{Dumps, Findings};

/// Checks the required reference containers and makes empty-family coverage explicit.
pub(super) fn validate(dumps: &Dumps, findings: &mut Findings) {
    for category in [
        "blueprints",
        "blueprint products",
        "blueprint slots",
        "blueprint modifiers",
        "contracts",
        "legacy contracts",
        "standings",
        "mining elements",
        "mining compositions",
        "mining cluster presets",
        "contract reward references",
    ] {
        findings.category(category);
    }
    for (name, value, kind) in [
        ("contracts", &dumps.merged["contracts"], Container::Array),
        (
            "legacyContracts",
            &dumps.merged["legacyContracts"],
            Container::Array,
        ),
        (
            "blueprints",
            &dumps.blueprints["blueprints"],
            Container::Array,
        ),
        (
            "mineableElements",
            &dumps.mining["mineableElements"],
            Container::Object,
        ),
        (
            "compositions",
            &dumps.mining["compositions"],
            Container::Object,
        ),
        (
            "clusteringPresets",
            &dumps.mining["clusteringPresets"],
            Container::Object,
        ),
    ] {
        findings.check("reference input", kind.accepts(value), || {
            format!("{name}: missing or wrong-shaped required container")
        });
    }
}

#[derive(Clone, Copy)]
enum Container {
    Array,
    Object,
}

impl Container {
    fn accepts(self, value: &Value) -> bool {
        match self {
            Self::Array => value.is_array(),
            Self::Object => value.is_object(),
        }
    }
}
