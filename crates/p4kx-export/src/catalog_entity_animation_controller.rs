// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::component_sources::physical_component;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::export_field_inspection::ConfigurationInspection;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

use super::ComponentKind;

pub(super) const KIND: ComponentKind = ComponentKind {
    source_type: "SAnimationControllerParams",
    inspection: ConfigurationInspection::EntityAnimationController,
};

physical_component!(EntityAnimationControllers, EntityAnimationController, KIND);

/// Mannequin file locators owned by one physical Entity component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct EntityAnimationController<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[serde(flatten)]
    source: UnitSource<'source>,
    animation_controller: SourcedValue<&'source str>,
    animation_database: SourcedValue<&'source str>,
}

impl<'source> EntityAnimationController<'source> {
    /// Borrows the physical owning Record and component ordinal.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the original component type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        self.source.source_type()
    }

    /// Borrows the original animation controller locator.
    #[must_use]
    pub const fn animation_controller(&self) -> &SourcedValue<&'source str> {
        &self.animation_controller
    }

    /// Borrows the original animation database locator.
    #[must_use]
    pub const fn animation_database(&self) -> &SourcedValue<&'source str> {
        &self.animation_database
    }

    /// Borrows original component members outside the locator fields.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        self.source.additional_fields()
    }

    pub(super) fn read(
        owner: ComponentOwner,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let animation_controller = fields.borrowed_string("AnimationController", notes)?;
        let animation_database = fields.borrowed_string("AnimationDatabase", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("AnimationController", animation_controller.state()),
                ("AnimationDatabase", animation_database.state()),
            ],
        )?;
        Ok(Self {
            owner,
            source,
            animation_controller,
            animation_database,
        })
    }
}
