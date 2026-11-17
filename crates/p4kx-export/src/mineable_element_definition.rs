// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::prepared_definition::PreparedDefinition;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::guid_text::CatalogGuid;
use crate::mining_locations::MiningRecordSource;
use crate::record_observation::RecordObservation;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// One original mineable element and its authored resource link and tuning.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct MineableElement {
    /// The sourced Record header.
    #[serde(flatten)]
    pub source: MiningRecordSource,
    /// The authored resource reference, including a retained dangling target.
    pub resource_type: SourcedValue<Box<CatalogRecordReference>>,
    /// The authored element instability.
    pub instability: SourcedValue<ResourceFloat32>,
    /// The authored element resistance.
    pub resistance: SourcedValue<ResourceFloat32>,
    /// The authored midpoint of the optimal window.
    pub optimal_window_midpoint: SourcedValue<ResourceFloat32>,
    /// The authored random variation of the midpoint.
    pub optimal_window_midpoint_randomness: SourcedValue<ResourceFloat32>,
    /// The authored width adjustment of the optimal window.
    pub optimal_window_thinness: SourcedValue<ResourceFloat32>,
    /// The authored explosion multiplier.
    pub explosion_multiplier: SourcedValue<ResourceFloat32>,
    /// The authored cluster factor.
    pub cluster_factor: SourcedValue<ResourceFloat32>,
}

impl MineableElement {
    /// Returns the referenced GUID when its Record is selected.
    #[must_use]
    pub fn resource_type_guid(&self) -> Option<Guid> {
        let reference = self.resource_type.value()?;
        reference.target_record_index.value()?;
        reference.guid.value().map(|guid| guid.0)
    }

    pub(crate) fn order(rows: &mut [Self]) {
        rows.sort_by_cached_key(|row| {
            (
                row.source
                    .guid
                    .value()
                    .map(|guid| CatalogGuid(*guid).text()),
                row.source.record_index.value().copied(),
            )
        });
    }
}

/// Reads one admitted element directly into its shared sourced representation.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(MineableElement, Vec<FieldError>), CatalogError> {
    let (prepared, notes) = PreparedDefinition::read(
        tree,
        projection,
        admission,
        cancel,
        |source, fields, _, notes| {
            Ok(MineableElement {
                source: source.into(),
                resource_type: fields
                    .record_link("resourceType", notes)?
                    .map(|link| Box::new((*link).into_catalog())),
                instability: fields.single("elementInstability", notes)?,
                resistance: fields.single("elementResistance", notes)?,
                optimal_window_midpoint: fields.single("elementOptimalWindowMidpoint", notes)?,
                optimal_window_midpoint_randomness: fields
                    .single("elementOptimalWindowMidpointRandomness", notes)?,
                optimal_window_thinness: fields.single("elementOptimalWindowThinness", notes)?,
                explosion_multiplier: fields.single("elementExplosionMultiplier", notes)?,
                cluster_factor: fields.single("elementClusterFactor", notes)?,
            })
        },
    )?;
    Ok((prepared.into_definition(), notes))
}

#[cfg(test)]
#[path = "mineable_element_definition_tests.rs"]
mod tests;
