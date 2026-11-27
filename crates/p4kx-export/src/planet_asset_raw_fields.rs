// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::entry_json_object::EntryJsonObject;
use crate::catalog_read::parsed_entry_json::SourceNode;
use crate::schema_fields::every_field_required;

/// An original unprojected JSON member with its own source evidence.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct RawPlanetAssetMember {
    #[schemars(extend("x-gameText" = true))]
    /// The exact authored member name token.
    pub name: SourcedValue<String>,
    #[schemars(extend("x-gameText" = true))]
    /// The exact authored value token.
    pub token: SourcedValue<String>,
}

pub(crate) fn collect(
    o: &EntryJsonObject<'_, '_>,
) -> Result<Vec<RawPlanetAssetMember>, CatalogError> {
    o.additional_fields(|name, token, sources| {
        let name_source = name.observe(SourceUseRole::Payload)?;
        let token = token_value(token, sources);
        Ok(RawPlanetAssetMember {
            name: sources.value(
                SourceValue::Present(name.raw().to_owned()),
                &[name_source],
                true,
            ),
            token,
        })
    })
}

pub(crate) fn token(node: &SourceNode<'_>) -> SourceValue<String> {
    SourceValue::Present(node.token.raw().to_owned())
}

fn token_value(node: &SourceNode<'_>, sources: &SourceUses) -> SourcedValue<String> {
    sources.value(token(node), std::slice::from_ref(&node.payload), true)
}
