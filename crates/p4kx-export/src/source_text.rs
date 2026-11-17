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
#[non_exhaustive]
pub enum SourceText<'source> {
    /// An original text classified in the public domain.
    Public {
        /// The exact original text or localization key.
        source_text: &'source str,
    },
    /// An original text classified in the protected game-text domain.
    GameText {
        /// The exact original text or localization key.
        #[schemars(extend("x-gameText" = true))]
        source_text: &'source str,
    },
}

impl<'source> SourceText<'source> {
    /// Borrows the exact original text or localization key.
    #[must_use]
    pub const fn as_str(&self) -> &'source str {
        match self {
            Self::Public { source_text } | Self::GameText { source_text } => source_text,
        }
    }

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
