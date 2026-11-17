// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::prepared_definition::PreparedDefinition;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::GameTextRef;
use crate::mining_locations::MiningRecordSource;
use crate::record_observation::RecordObservation;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// A mineable composition and its authored element shares.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct MineableComposition {
    /// The sourced Record header.
    #[serde(flatten)]
    pub source: MiningRecordSource,
    /// The protected HUD label from the original `depositName` Locale.
    #[schemars(extend("x-gameText" = true))]
    pub deposit_name: SourcedValue<GameTextRef>,
    /// The authored minimum number of distinct elements.
    pub minimum_distinct_elements: SourcedValue<ResourceNumber>,
    /// The members of `compositionArray` at their original zero-based positions.
    pub parts: SourcedValue<Vec<SourcedValue<CompositionPart>>>,
}

impl MineableComposition {
    pub(crate) fn order(rows: &mut [Self]) {
        rows.sort_by_cached_key(|row| row.source.order_key());
    }
}

/// An original composition member with independently sourced reference and tuning fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct CompositionPart {
    /// The authored element reference, including a retained dangling target.
    pub mineable_element: SourcedValue<Box<CatalogRecordReference>>,
    /// The minimum share expressed as a percentage.
    pub min_percentage: SourcedValue<ResourceNumber>,
    /// The maximum share expressed as a percentage.
    pub max_percentage: SourcedValue<ResourceNumber>,
    /// The probability that the part occurs.
    pub probability: SourcedValue<ResourceNumber>,
    /// The exponent of the share distribution curve.
    pub curve_exponent: SourcedValue<ResourceNumber>,
    /// The scale applied to the element quality.
    pub quality_scale: SourcedValue<ResourceNumber>,
}

impl CompositionPart {
    fn read(
        fields: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            mineable_element: fields
                .record_link("mineableElement", notes)?
                .map(|link| Box::new((*link).into_catalog())),
            min_percentage: fields.authored_number("minPercentage", notes)?,
            max_percentage: fields.authored_number("maxPercentage", notes)?,
            probability: fields.authored_number("probability", notes)?,
            curve_exponent: fields.authored_number("curveExponent", notes)?,
            quality_scale: fields.authored_number("qualityScale", notes)?,
        })
    }
}

pub(crate) struct CompositionRead {
    pub(crate) value: MineableComposition,
    pub(crate) notes: Vec<FieldError>,
    pub(crate) unreadable_text_fields: usize,
}

/// Reads a composition through its original Record and declaration observations.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<CompositionRead, CatalogError> {
    let mut unreadable_text_fields = 0;
    let (prepared, notes) = PreparedDefinition::read(
        tree,
        projection,
        admission,
        cancel,
        |source, fields, projection, notes| {
            let deposit_name = fields.protected_locale("depositName", projection, notes)?;
            unreadable_text_fields = notes.len();
            Ok(MineableComposition {
                source: source.into(),
                deposit_name,
                minimum_distinct_elements: fields
                    .authored_integer("minimumDistinctElements", notes)?,
                parts: fields.declared_class_array(
                    "compositionArray",
                    notes,
                    CompositionPart::read,
                )?,
            })
        },
    )?;
    Ok(CompositionRead {
        value: prepared.into_definition(),
        notes,
        unreadable_text_fields,
    })
}
