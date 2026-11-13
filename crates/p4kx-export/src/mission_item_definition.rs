// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::prepared_definition::PreparedDefinition;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_table::CatalogTable;
use crate::catalog_text_contract::GameTextRef;
use crate::record_observation::RecordObservation;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};
use crate::{CatalogError, ExportAccountingError, Kind};

pub(crate) const SOURCE_TYPE: &str = "MissionItem";
const VARIANTS: &str = "MissionStringVariants";
const VARIANT: &str = "MissionStringVariant";
type RecordLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;

pub(crate) type PreparedMissionItem<'source> =
    PreparedDefinition<'source, MissionItemDefinition<'source>>;

/// A physical mission item with authored entity, weight and localized variants.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionItemDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    tags: SourcedValue<Vec<RecordLink<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    entity_class: RecordLink<'source>,
    weighting: SourcedValue<ResourceFloat32>,
    string_variants: SourcedValue<MissionStringVariants<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct MissionStringVariants<'source> {
    variants: SourcedValue<Vec<SourcedValue<MissionStringVariant<'source>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct MissionStringVariant<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    tag: RecordLink<'source>,
    #[schemars(extend("x-gameText" = true))]
    string: SourcedValue<GameTextRef>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> MissionItemDefinition<'source> {
    fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let tags = fields.record_link_array_to("tags", "Tag", notes)?;
        let entity_class = fields.record_link_to("entityClass", "EntityClassDefinition", notes)?;
        let weighting = fields.single("weighting", notes)?;
        let string_variants =
            fields.declared_class_to("stringVariants", VARIANTS, notes, |fields, notes| {
                MissionStringVariants::read(fields, projection, notes)
            })?;
        let authored_fields = fields.authored_members_except([
            ("tags", tags.state()),
            ("entityClass", entity_class.state()),
            ("weighting", weighting.state()),
            ("stringVariants", string_variants.state()),
        ])?;
        Ok(Self {
            source,
            tags,
            entity_class,
            weighting,
            string_variants,
            authored_fields,
        })
    }
}

impl<'source> MissionStringVariants<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let variants = fields.class_array_to("variants", VARIANT, notes, |fields, notes| {
            MissionStringVariant::read(fields, projection, notes)
        })?;
        let authored_fields = fields.authored_members_except([("variants", variants.state())])?;
        Ok(Self {
            variants,
            authored_fields,
        })
    }
}

impl<'source> MissionStringVariant<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let tag = fields.record_link_to("tag", "Tag", notes)?;
        let string = fields.protected_locale("string", projection, notes)?;
        let authored_fields =
            fields.authored_members_except([("tag", tag.state()), ("string", string.state())])?;
        Ok(Self {
            tag,
            string,
            authored_fields,
        })
    }
}

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(PreparedMissionItem<'source>, Vec<FieldError>), CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    PreparedDefinition::read(
        tree,
        projection,
        admission,
        cancel,
        MissionItemDefinition::read,
    )
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<PreparedMissionItem<'source>>,
) -> CatalogTable<MissionItemDefinition<'source>> {
    PreparedDefinition::order(&mut rows);
    PreparedDefinition::table(forge, SOURCE_TYPE, rows)
}

pub(crate) fn sources<'rows, 'source>(
    rows: &'rows [PreparedMissionItem<'source>],
) -> impl Iterator<Item = (Kind, Guid, &'source str, &'rows [CapturedNumber])> {
    PreparedDefinition::sources(Kind::MissionItems, rows)
}
