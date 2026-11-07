// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, RecordTree};
use thiserror::Error;

use crate::blueprint::as_struct;
use crate::crafting_types::{QualityQuantization, QuantizationBand};
use crate::record_fields::{class_name, elements, integer, reference, structure};
use crate::selection::{Kind, KindRule};

pub(crate) const QUANTIZATIONS: KindRule = Kind::QualityQuantizations.records();
pub(crate) const RESOURCE_TYPES: KindRule = Kind::ResourceTypes.records();

/// Why a `CraftingQualityQuantizationRecord` cannot be read.
#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum Malformed {
    #[error("the Record holds no qualityQuantization")]
    NoQuantization,
    #[error("the Record holds no bands")]
    NoBands,
    #[error("bands[{0}] lacks a whole number start, end or mappedValue")]
    Incomplete(usize),
    #[error("bands[{0}] ends before it starts")]
    Reversed(usize),
}

/// Returns the quality quantization the Record tree `tree` describes, without the `ResourceType`s
/// that use it.
///
/// # Errors
///
/// Returns [`Malformed`] for a Record without bands or with a band that lacks a whole number or
/// ends before it starts.
pub(crate) fn quantization<'f>(
    tree: &RecordTree<'f>,
) -> Result<QualityQuantization<'f>, Malformed> {
    let body =
        structure(Some(tree.root()), "qualityQuantization").ok_or(Malformed::NoQuantization)?;
    let stored = elements(Some(body), "bands");
    if stored.is_empty() {
        return Err(Malformed::NoBands);
    }
    let mut bands = Vec::with_capacity(stored.len());
    for (index, value) in stored.iter().enumerate() {
        let band = as_struct(value);
        let (Some(start), Some(end), Some(mapped_value)) = (
            integer(band, "start"),
            integer(band, "end"),
            integer(band, "mappedValue"),
        ) else {
            return Err(Malformed::Incomplete(index));
        };
        if end < start {
            return Err(Malformed::Reversed(index));
        }
        bands.push(QuantizationBand {
            start,
            end,
            mapped_value,
        });
    }
    Ok(QualityQuantization {
        guid: tree.guid(),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        bands,
        resource_types: Vec::new(),
    })
}

/// Returns the GUIDs of the quality quantization Records that the crafting data of the
/// `ResourceType` Record tree `tree` link to.
pub(crate) fn linked_quantizations(tree: &RecordTree<'_>) -> Vec<Guid> {
    elements(Some(tree.root()), "properties")
        .iter()
        .filter_map(as_struct)
        .filter_map(|data| {
            reference(
                structure(Some(data), "qualityQuantization"),
                "qualityQuantizationRecord",
            )
        })
        .map(|guid| guid.0)
        .collect()
}
