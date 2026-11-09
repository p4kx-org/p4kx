// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, StructValue, Value};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::CatalogRecordContext;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};

use super::contract::{QuantizationEligibility, QuantizationIneligibility};
use super::evidence::Evidence;

pub(crate) struct Eligibility {
    pub(crate) result: QuantizationEligibility,
    pub(crate) evidence: Evidence,
    pub(crate) admission: Evidence,
}

pub(crate) fn read(
    tree: &RecordTree<'_>,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<Eligibility, CatalogError> {
    let context = origins.record(tree.record_index());
    let mut evidence = Evidence::default();
    let value = tree.field("qualityQuantization");
    evidence.field(&context, "qualityQuantization", value)?;
    let Some(Value::Struct(source)) = value else {
        return Ok(excluded(evidence, QuantizationIneligibility::NoInlineBands));
    };
    evidence.field(&context, "qualityQuantization.$type", None)?;
    if source.type_name() != "CraftingQualityQuantization" {
        return Ok(excluded(evidence, QuantizationIneligibility::NoInlineBands));
    }
    let value = source.field("bands");
    evidence.field(&context, "qualityQuantization.bands", value)?;
    let Some(Value::Array(bands)) = value else {
        return Ok(excluded(evidence, QuantizationIneligibility::InvalidBands));
    };
    if bands.is_empty() {
        return Ok(excluded(evidence, QuantizationIneligibility::EmptyBands));
    }
    if !valid_bands(bands, &context, &mut evidence, cancel)? {
        return Ok(excluded(evidence, QuantizationIneligibility::InvalidBands));
    }
    evidence.header(
        &context,
        RecordHeaderInput::Field(RecordHeaderField::Guid),
        true,
    )?;
    Ok(Eligibility {
        result: QuantizationEligibility::Eligible,
        evidence,
        admission: Evidence::default(),
    })
}

fn valid_bands(
    bands: &[Value<'_>],
    context: &CatalogRecordContext<'_>,
    evidence: &mut Evidence,
    cancel: Option<&CancelToken>,
) -> Result<bool, CatalogError> {
    for (ordinal, value) in bands.iter().enumerate() {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let path = format!("qualityQuantization.bands[{ordinal}]");
        evidence.field(context, &path, Some(value))?;
        let Some(source) = band(value) else {
            return Ok(false);
        };
        if !valid_band(source, &path, context, evidence)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn valid_band(
    source: &StructValue<'_>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    evidence: &mut Evidence,
) -> Result<bool, CatalogError> {
    evidence.field(context, &format!("{path}.$type"), None)?;
    if source.type_name() != "CraftingQualityQuantizationBand" {
        return Ok(false);
    }
    for field in ["start", "end", "mappedValue"] {
        evidence.field(context, &format!("{path}.{field}"), source.field(field))?;
    }
    let (Some(Value::Int32(start)), Some(Value::Int32(end)), Some(Value::Int32(_))) = (
        source.field("start"),
        source.field("end"),
        source.field("mappedValue"),
    ) else {
        return Ok(false);
    };
    Ok(end >= start)
}

const fn band<'a, 'f>(value: &'a Value<'f>) -> Option<&'a StructValue<'f>> {
    if let Value::Struct(source) = value {
        Some(source)
    } else {
        None
    }
}

fn excluded(evidence: Evidence, reason: QuantizationIneligibility) -> Eligibility {
    Eligibility {
        result: QuantizationEligibility::NotEligible(reason),
        evidence,
        admission: Evidence::default(),
    }
}
