// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;

/// The exact original string or key with its public or protected text domain.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum SourceText<'source> {
    Public {
        source_text: &'source str,
    },
    GameText {
        #[schemars(extend("x-gameText" = true))]
        source_text: &'source str,
    },
}

impl<'source> SourceText<'source> {
    /// Protects unclassified domains while retaining known public facts unchanged.
    pub(crate) fn read(owner: &str, field: &str, text: &'source str) -> Self {
        match (owner, field) {
            ("PoolFilterInstance", "name" | "mode")
            | ("CraftingGameplayPropertyDef", "propertyName" | "unitFormat") => {
                Self::Public { source_text: text }
            }
            _ => Self::GameText { source_text: text },
        }
    }
}
