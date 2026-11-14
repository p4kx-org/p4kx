// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::PublicNameRef;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(crate) const SOURCE_TYPE: &str = "ContractScopeGlobalParams";
const ITEM_TYPE: &str = "ContractScope_Item";

/// One physical declaration of the contract scope labels and icons.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ContractScopeGlobalParams<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    location: SourcedValue<ContractScopeItem<'source>>,
    cluster: SourcedValue<ContractScopeItem<'source>>,
    sector: SourcedValue<ContractScopeItem<'source>>,
    planetary_body: SourcedValue<ContractScopeItem<'source>>,
    planetary_system: SourcedValue<ContractScopeItem<'source>>,
    solar_system: SourcedValue<ContractScopeItem<'source>>,
    universe: SourcedValue<ContractScopeItem<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ContractScopeGlobalParams<'source> {
    pub(in crate::catalog_v3) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut scope = |name| {
            fields.strong_pointer_family_to(name, ITEM_TYPE, notes, |item, notes| {
                ContractScopeItem::read(item, projection, notes)
            })
        };
        let location = scope("location")?;
        let cluster = scope("cluster")?;
        let sector = scope("sector")?;
        let planetary_body = scope("planetaryBody")?;
        let planetary_system = scope("planetarySystem")?;
        let solar_system = scope("solarSystem")?;
        let universe = scope("universe")?;
        let additional_fields = fields.authored_members_except([
            ("location", location.state()),
            ("cluster", cluster.state()),
            ("sector", sector.state()),
            ("planetaryBody", planetary_body.state()),
            ("planetarySystem", planetary_system.state()),
            ("solarSystem", solar_system.state()),
            ("universe", universe.state()),
        ])?;
        Ok(Self {
            source,
            location,
            cluster,
            sector,
            planetary_body,
            planetary_system,
            solar_system,
            universe,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ContractScopeItem<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    display_name: SourcedValue<PublicNameRef>,
    icon: SourcedValue<&'source str>,
}

impl<'source> ContractScopeItem<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let display_name = fields.public_name("displayName", projection, notes)?;
        let icon = fields.borrowed_string("icon", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("displayName", display_name.state()),
                ("icon", icon.state()),
            ],
        )?;
        Ok(Self {
            source,
            display_name,
            icon,
        })
    }
}

#[cfg(test)]
#[path = "contract_scope_global_params_tests.rs"]
mod tests;
