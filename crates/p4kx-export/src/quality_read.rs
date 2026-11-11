// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree};

use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::NumberCapture;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::CatalogFields;
use crate::guid_text::CatalogGuid;
use crate::quality_source_read;
use crate::source_fields::{FieldCapture, FieldError};

use super::contract::{QualityDistributionDefinition, QualityOwner, QualityQuantizationDefinition};

pub(crate) struct Read<T> {
    pub(crate) value: T,
    pub(crate) notes: Vec<FieldError>,
}

pub(crate) fn distribution<'f>(
    tree: &RecordTree<'f>,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
    capture: &dyn FieldCapture<'f>,
) -> Result<Read<QualityDistributionDefinition<'f>>, CatalogError> {
    let numbers = NumberCapture::default();
    let context = origins
        .record(tree.record_index())
        .with_numbers(&numbers)
        .with_cancel(cancel);
    let fields = CatalogFields::observed(tree.root(), "", &context, capture)?;
    let mut notes = Vec::new();
    fields.check_observed(
        if tree.type_name() == "CraftingQualityDistributionRecord" {
            &["qualityDistribution"]
        } else {
            &["locationOverride"]
        },
        &mut notes,
    )?;
    let quality_distribution = fields
        .structure("qualityDistribution", &mut notes)?
        .try_map(|source| {
            quality_source_read::distribution(
                source,
                "qualityDistribution",
                &context,
                &mut notes,
                capture,
            )
        })?;
    let location_override =
        fields
            .structure("locationOverride", &mut notes)?
            .try_map(|source| {
                quality_source_read::overrides(
                    source,
                    "locationOverride",
                    &context,
                    &mut notes,
                    capture,
                )
            })?;
    Ok(Read {
        value: QualityDistributionDefinition {
            owner: owner(tree),
            numbers: numbers.finish(),
            source: CanonicalRecordHeader::read(tree, origins)?,
            quality_distribution,
            location_override,
        },
        notes,
    })
}

pub(crate) fn quantization<'f>(
    tree: &RecordTree<'f>,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
    capture: &dyn FieldCapture<'f>,
) -> Result<Read<QualityQuantizationDefinition<'f>>, CatalogError> {
    let numbers = NumberCapture::default();
    let context = origins
        .record(tree.record_index())
        .with_numbers(&numbers)
        .with_cancel(cancel);
    let fields = CatalogFields::observed(tree.root(), "", &context, capture)?;
    let mut notes = Vec::new();
    fields.check_observed(&["qualityQuantization"], &mut notes)?;
    let quality_quantization = fields
        .structure("qualityQuantization", &mut notes)?
        .try_map(|source| {
            quality_source_read::quantization(
                source,
                "qualityQuantization",
                &context,
                &mut notes,
                capture,
            )
        })?;
    Ok(Read {
        value: QualityQuantizationDefinition {
            owner: owner(tree),
            numbers: numbers.finish(),
            source: CanonicalRecordHeader::read(tree, origins)?,
            quality_quantization,
        },
        notes,
    })
}

pub(crate) const fn owner<'f>(tree: &RecordTree<'f>) -> QualityOwner<'f> {
    QualityOwner {
        guid: CatalogGuid(tree.guid()),
        record_index: tree.record_index(),
        record_path: tree.record_path(),
    }
}
