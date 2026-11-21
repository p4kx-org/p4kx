// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_text_contract::PublicNameRef;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_item_components::source_getters;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::prepared_definition::PreparedDefinition;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_field_walk;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(crate) const SOURCE_TYPE: &str = "ContractScopeGlobalParams";
const ITEM_TYPE: &str = "ContractScope_Item";

/// One physical declaration of the contract scope labels and icons.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ContractScopeGlobalParams<'source> {
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

pub(super) fn prepare<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<
    (
        PreparedDefinition<'source, ContractScopeGlobalParams<'source>>,
        Vec<FieldError>,
    ),
    CatalogError,
> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    PreparedDefinition::read(
        tree,
        projection,
        admission,
        cancel,
        ContractScopeGlobalParams::read,
    )
}

pub(crate) fn read_selected<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<ContractScopeGlobalParams<'source>>, CatalogError> {
    let (row, notes) = prepare(tree, projection, admission, cancel)?;
    let inspection = export_field_walk::inspect_observed(
        Some(projection.original()),
        tree,
        admission,
        Some(Kind::ContractScopes),
        ExportSourceRole::Selected,
        cancel,
    )?;
    Ok(ScopedSourceRead::new(
        row.into_definition(),
        notes,
        inspection.field_projection_complete(0, 0),
    ))
}

impl<'source> ContractScopeGlobalParams<'source> {
    source_getters! {
        /// Borrows the original physical scope Record header.
        source: CanonicalRecordHeader<'source>,
        /// Borrows the original location scope slot.
        location: SourcedValue<ContractScopeItem<'source>>,
        /// Borrows the original cluster scope slot.
        cluster: SourcedValue<ContractScopeItem<'source>>,
        /// Borrows the original sector scope slot.
        sector: SourcedValue<ContractScopeItem<'source>>,
        /// Borrows the original planetary body scope slot.
        planetary_body: SourcedValue<ContractScopeItem<'source>>,
        /// Borrows the original planetary system scope slot.
        planetary_system: SourcedValue<ContractScopeItem<'source>>,
        /// Borrows the original solar system scope slot.
        solar_system: SourcedValue<ContractScopeItem<'source>>,
        /// Borrows the original universe scope slot.
        universe: SourcedValue<ContractScopeItem<'source>>,
        /// Borrows other members of the original scope definition.
        additional_fields: SourceArenaOutput<'source>,
    }

    fn read(
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

/// The label and icon of one original contract scope declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ContractScopeItem<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    display_name: SourcedValue<PublicNameRef>,
    icon: SourcedValue<&'source str>,
}

impl<'source> ContractScopeItem<'source> {
    source_getters! {
        /// Borrows the original concrete source type and remaining members.
        source: UnitSource<'source>,
        /// Borrows the original public name reference.
        display_name: SourcedValue<PublicNameRef>,
        /// Borrows the original icon locator.
        icon: SourcedValue<&'source str>,
    }

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
