// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use super::location_vocabulary::RESOURCE_SLOT;
use super::prepared_definition::PreparedDefinition;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::GameTextRef;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(crate) const SOURCE_TYPE: &str = "LocationEntityDeclaration";
const TARGET_TYPE: &str = "LocationEntityType_Base";
const DELIVERY_TYPE: &str = "DeliveryObjectiveType";

type RecordLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;
type Tags<'source> = SourcedValue<Vec<RecordLink<'source>>>;

/// A physical location entity declaration with its authored tags and delivery type.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct LocationEntityDeclaration<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    entity_tags: SourcedValue<LocationEntityTags<'source>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    resource_slot: RecordLink<'source>,
    #[serde(rename = "type")]
    entity_type: SourcedValue<LocationEntityType<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> LocationEntityDeclaration<'source> {
    /// Returns the physical Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Returns the original entity tag block.
    #[must_use]
    pub const fn entity_tags(&self) -> &SourcedValue<LocationEntityTags<'source>> {
        &self.entity_tags
    }

    /// Returns the original resource-slot link.
    #[must_use]
    pub const fn resource_slot(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.resource_slot
    }

    /// Returns the original location entity type.
    #[must_use]
    pub const fn entity_type(&self) -> &SourcedValue<LocationEntityType<'source>> {
        &self.entity_type
    }

    /// Returns the original members outside the typed fields.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }

    fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let entity_tags =
            fields.declared_class_to("entityTags", "TagList", notes, LocationEntityTags::read)?;
        let resource_slot = fields.record_link_to("resourceSlot", RESOURCE_SLOT, notes)?;
        let entity_type =
            fields.strong_pointer_family_to("type", TARGET_TYPE, notes, |fields, notes| {
                LocationEntityType::read(fields, projection, notes)
            })?;
        let authored_fields = fields.authored_members_except([
            ("entityTags", entity_tags.state()),
            ("resourceSlot", resource_slot.state()),
            ("type", entity_type.state()),
        ])?;
        Ok(Self {
            source,
            entity_tags,
            resource_slot,
            entity_type,
            authored_fields,
        })
    }
}

/// The original entity tag block and its ordered Reference slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct LocationEntityTags<'source> {
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    tags: Tags<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> LocationEntityTags<'source> {
    /// Returns the original tag Reference array in stored order.
    #[must_use]
    pub const fn tags(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>> {
        &self.tags
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let tags = fields.record_link_array_to("tags", "Tag", notes)?;
        let authored_fields = fields.authored_members_except([("tags", tags.state())])?;
        Ok(Self {
            tags,
            authored_fields,
        })
    }
}

/// The original location entity type and its kind-specific fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct LocationEntityType<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    kind: LocationEntityKind<'source>,
}

impl<'source> LocationEntityType<'source> {
    /// Returns the original concrete struct name.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        self.source.source_type()
    }

    /// Returns the original members outside the typed fields.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        self.source.additional_fields()
    }

    /// Returns the typed interpretation of the concrete declaration.
    #[must_use]
    pub const fn kind(&self) -> &LocationEntityKind<'source> {
        &self.kind
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let (kind, source) = match fields.source_type()? {
            "LocationEntityType_StaticEntity" => (
                LocationEntityKind::StaticEntity,
                UnitSource::read(fields, [])?,
            ),
            "LocationEntityType_DeliveryTarget" => {
                let delivery_objective_type =
                    fields.borrowed_enum_to("type", DELIVERY_TYPE, notes)?;
                let target_description =
                    fields.protected_locale("targetDescription", projection, notes)?;
                let source = UnitSource::read(
                    fields,
                    [
                        ("type", delivery_objective_type.state()),
                        ("targetDescription", target_description.state()),
                    ],
                )?;
                (
                    LocationEntityKind::DeliveryTarget {
                        delivery_objective_type,
                        target_description,
                    },
                    source,
                )
            }
            _ => (LocationEntityKind::Original, UnitSource::read(fields, [])?),
        };
        Ok(Self { source, kind })
    }
}

/// The typed interpretation of a location entity type.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum LocationEntityKind<'source> {
    /// The original static-entity declaration.
    StaticEntity,
    /// The original delivery objective settings.
    DeliveryTarget {
        /// The option named by the original `DeliveryObjectiveType` field.
        delivery_objective_type: SourcedValue<&'source str>,
        /// The original player-facing description.
        #[schemars(extend("x-gameText" = true))]
        target_description: SourcedValue<GameTextRef>,
    },
    /// Another concrete source type whose fields remain in the original arena.
    Original,
}

pub(crate) fn prepare<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<
    (
        PreparedDefinition<'source, LocationEntityDeclaration<'source>>,
        Vec<FieldError>,
    ),
    CatalogError,
> {
    PreparedDefinition::read(
        tree,
        projection,
        admission,
        cancel,
        LocationEntityDeclaration::read,
    )
}
