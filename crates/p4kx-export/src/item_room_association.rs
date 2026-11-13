// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(in crate::catalog_v3) const COMPONENT: &str = "SObjectContainerComponentParams";

/// The original scalar object container of one physical item component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ItemRoomAssociation<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    object_container: SourcedValue<&'source str>,
    #[serde(skip)]
    pub(in crate::catalog_v3) file_use: Option<SourceUse>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ItemRoomAssociation<'source> {
    pub(in crate::catalog_v3) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let object_container = source.borrowed_string("objectContainer", notes)?;
        let additional_fields =
            source.authored_members_except([("objectContainer", object_container.state())])?;
        Ok(Self {
            owner,
            object_container,
            file_use: source.selector_use("objectContainer")?,
            additional_fields,
        })
    }

    pub(in crate::catalog_v3) const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    pub(in crate::catalog_v3) const fn file_name(&self) -> Option<&'source str> {
        self.object_container.value().copied()
    }
}
