// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, StructValue, Value};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::CatalogRecordContext;
use crate::catalog_read::original::OriginalFlowFields;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};

use super::contract::{QuantizationEligibility, QuantizationIneligibility};
use super::declaration_survey;
use super::evidence::Evidence;

pub(crate) struct Eligibility {
    pub(crate) result: QuantizationEligibility,
    pub(crate) evidence: Evidence,
    pub(crate) admission: Evidence,
}

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    origins: &CatalogOrigins,
    original: &OriginalFlowFields<'_, '_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<Eligibility, CatalogError> {
    original.check_tree(tree)?;
    let context = origins.record(tree.record_index());
    original.check_context(context.sources(), context.dataforge(), tree.record_index())?;
    let mut evidence = Evidence::default();
    let root = tree.root();
    let value = evidence.lookup(
        &context,
        root,
        "qualityQuantization",
        "qualityQuantization",
        || declaration_survey::survey(original, root),
    )?;
    let Some(Value::Struct(source)) = value else {
        return Ok(excluded(evidence, QuantizationIneligibility::NoInlineBands));
    };
    if evidence.concrete_type(&context, "qualityQuantization", source)?
        != "CraftingQualityQuantization"
    {
        return Ok(excluded(evidence, QuantizationIneligibility::NoInlineBands));
    }
    let value = evidence.lookup(
        &context,
        source,
        "bands",
        "qualityQuantization.bands",
        || declaration_survey::survey(original, source),
    )?;
    let Some(Value::Array(bands)) = value else {
        return Ok(excluded(evidence, QuantizationIneligibility::InvalidBands));
    };
    if bands.is_empty() {
        return Ok(excluded(evidence, QuantizationIneligibility::EmptyBands));
    }
    if !valid_bands(bands, &context, &mut evidence, original, cancel)? {
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

fn valid_bands<'source>(
    bands: &[Value<'source>],
    context: &CatalogRecordContext<'_>,
    evidence: &mut Evidence,
    original: &OriginalFlowFields<'_, '_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<bool, CatalogError> {
    for (ordinal, value) in bands.iter().enumerate() {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let path = format!("qualityQuantization.bands[{ordinal}]");
        evidence.field(context, &path, value)?;
        let Some(source) = band(value) else {
            return Ok(false);
        };
        if !valid_band(source, &path, context, evidence, original)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn valid_band<'source>(
    source: &StructValue<'source>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    evidence: &mut Evidence,
    original: &OriginalFlowFields<'_, '_, 'source>,
) -> Result<bool, CatalogError> {
    if evidence.concrete_type(context, path, source)? != "CraftingQualityQuantizationBand" {
        return Ok(false);
    }
    let mut field = |name| {
        evidence.lookup(context, source, name, &format!("{path}.{name}"), || {
            declaration_survey::survey(original, source)
        })
    };
    let (Some(Value::Int32(start)), Some(Value::Int32(end)), Some(Value::Int32(_))) =
        (field("start")?, field("end")?, field("mappedValue")?)
    else {
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
