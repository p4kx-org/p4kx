// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_audio::{
    BankBlendContainerFields, BankContainerFields, BankHircContainerNode, BankHircObject,
    BankObservedHeader,
};
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use crate::source_evidence_error::SourceEvidenceError;

#[path = "hirc_blend_source_dto.rs"]
mod dto;
#[path = "hirc_blend_source_rows.rs"]
mod rows;

pub(super) use self::dto::BlendSourceSchema;
use self::dto::{BlendFields, BlendSourceDocument, BlendSourceKind, BlendSourceRow};
use self::rows::BlendRows;
use super::container_layout::{self, CheckedExtent, ContainerSourceLayout};
use super::origins::ObjectOrigins;

pub(super) fn serialize<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    container: BankHircContainerNode<'_, R>,
) -> Result<S::Ok, S::Error> {
    let extent = CheckedExtent::observe(
        container.fields().blend.as_ref(),
        BankBlendContainerFields::complete_range,
    );
    let (layout, error) = container_layout::coverage(container.status(), extent);
    let original = container.object();
    BlendSourceDocument {
        kind: BlendSourceKind::BlendContainer,
        layout: origins
            .coverage(layout, header, original)
            .map_err(S::Error::custom)?,
        version: origins.version(header).map_err(S::Error::custom)?,
        error: error
            .map(|error| origins.coverage(error.to_string(), header, original))
            .transpose()
            .map_err(S::Error::custom)?,
        fields: observations(container.fields(), origins).map_err(S::Error::custom)?,
        rows: BlendRows::new(container, origins),
        source_inventory: origins,
    }
    .serialize(serializer)
}

pub(super) fn unsupported<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    original: &BankHircObject,
) -> Result<S::Ok, S::Error> {
    BlendSourceDocument {
        kind: BlendSourceKind::BlendContainer,
        layout: origins
            .coverage(ContainerSourceLayout::Unsupported, header, original)
            .map_err(S::Error::custom)?,
        version: origins.version(header).map_err(S::Error::custom)?,
        error: None,
        fields: BlendFields::default(),
        rows: [] as [BlendSourceRow; 0],
        source_inventory: origins,
    }
    .serialize(serializer)
}

fn observations(
    container: &BankContainerFields,
    origins: &ObjectOrigins,
) -> Result<BlendFields, SourceEvidenceError> {
    let child_range = container
        .children
        .as_ref()
        .map(|range| origins.range(range, "containerChildrenExtent"))
        .transpose()?;
    let (layers, continuous_validation) = container.blend.as_ref().map_or((None, None), |fields| {
        (
            fields.layers.as_ref(),
            fields.continuous_validation.as_ref(),
        )
    });
    Ok(BlendFields {
        child_count: origins.optional(container.child_count.as_ref(), "containerChildCount")?,
        child_range,
        layers: rows::table(origins, layers, "blendLayerCount", "blendLayersExtent")?,
        continuous_validation: origins
            .optional(continuous_validation, "blendContinuousValidation")?,
    })
}
