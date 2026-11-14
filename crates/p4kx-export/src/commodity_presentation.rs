// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceUse, SourceValue, SourcedValue};

use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::commodity_contract::CommodityPresentation;
use crate::commodity_resource_presentation;
use crate::commodity_source;
use crate::commodity_text_choice;
use crate::commodity_text_input::CommodityTextInput;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::resource_group::Groups;
use crate::source_fields::FieldError;

/// The already-read inputs of the established Commodity presentation recipes.
pub(crate) struct CommodityPresentationContext<'read, 'context, 'source> {
    projection: &'read CatalogProjection<'context, 'source>,
    resource: Option<&'read RecordTree<'source>>,
    links: &'read [SourceUse],
    resource_inputs: &'read [SourceUse],
    resource_complete: bool,
}

pub(crate) struct CommodityPresentationTexts {
    name: SourcedValue<Option<PublicNameRef>>,
    description: SourcedValue<Option<GameTextRef>>,
    display_type: SourcedValue<Option<GameTextRef>>,
}

impl<'read, 'context, 'source> CommodityPresentationContext<'read, 'context, 'source> {
    pub(crate) const fn new(
        projection: &'read CatalogProjection<'context, 'source>,
        resource: Option<&'read RecordTree<'source>>,
        links: &'read [SourceUse],
        resource_inputs: &'read [SourceUse],
        resource_complete: bool,
    ) -> Self {
        Self {
            projection,
            resource,
            links,
            resource_inputs,
            resource_complete,
        }
    }

    pub(crate) fn texts(
        &self,
        own_name: Option<&CommodityTextInput<'source>>,
        own_description: Option<&CommodityTextInput<'source>>,
        display_type: Option<&CommodityTextInput<'source>>,
        notes: &mut Vec<FieldError>,
    ) -> Result<CommodityPresentationTexts, CatalogError> {
        let name = self.fallback(own_name, "displayName", notes)?;
        let description = self.fallback(own_description, "description", notes)?;
        Ok(CommodityPresentationTexts {
            name: commodity_text_choice::name(
                self.projection,
                own_name,
                name.as_ref(),
                self.resource_inputs,
                self.resource_complete,
            )?,
            description: commodity_text_choice::game_text(
                self.projection,
                own_description,
                description.as_ref(),
                self.resource_inputs,
                self.resource_complete,
            )?,
            display_type: commodity_text_choice::game_text(
                self.projection,
                display_type,
                None,
                &[],
                true,
            )?,
        })
    }

    fn fallback(
        &self,
        own: Option<&CommodityTextInput<'source>>,
        field: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<Option<CommodityTextInput<'source>>, CatalogError> {
        if !commodity_text_choice::needs_fallback(own) {
            return Ok(None);
        }
        self.resource
            .map(|tree| commodity_source::resource_text(tree, field, self.projection, notes))
            .transpose()
    }

    pub(crate) fn finish(
        &self,
        tree: &RecordTree<'source>,
        texts: CommodityPresentationTexts,
        subtype_guid: SourcedValue<Option<CatalogGuid>>,
        groups: &Groups,
    ) -> Result<CommodityPresentation, CatalogError> {
        let origins = self.projection.origins();
        let subtype = subtype_guid.value().copied().flatten();
        Ok(CommodityPresentation {
            commodity_guid: origins.record(tree.record_index()).header_value(
                RecordHeaderField::Guid,
                SourceValue::Present(CatalogGuid(tree.guid())),
            )?,
            name: texts.name,
            description: texts.description,
            display_type: texts.display_type,
            type_guid: commodity_resource_presentation::first_group(
                origins, subtype, self.links, groups,
            )?,
            subtype_guid,
            density_grams_per_cubic_centimetre: commodity_resource_presentation::density(
                origins,
                self.resource,
                self.resource_inputs,
                self.resource_complete,
            )?,
            refined_subtype_guid: commodity_resource_presentation::refined(
                origins,
                self.resource,
                self.resource_inputs,
                self.resource_complete,
            )?,
        })
    }
}
