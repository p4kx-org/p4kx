// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_extraction::{CancelToken, SourceValue, SourcedValue};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};

use super::assembly::Quantizer;
use super::contract::{QualityOwner, QualityQuantizationResources, QuantizationEligibility};
use super::evidence::Evidence;
use super::fallbacks::Fallbacks;
use super::link_index::{Links, Members};
use super::{QualityRows, Retention, check_cancel};

pub(super) fn fill<'source>(
    rows: &mut QualityRows<'source>,
    quantizers: &[Quantizer<'source>],
    mut links: Links,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    let mut used = HashSet::new();
    for quantizer in quantizers {
        check_cancel(cancel)?;
        let first = eligible(quantizer) && used.insert(quantizer.guid);
        let members = if first {
            links.take(quantizer.guid)
        } else {
            Members::new()
        };
        if matches!(quantizer.retention, Retention::Leftover) && first {
            rows.summary.unresolved_links += members.len();
        }
        if matches!(quantizer.retention, Retention::Selected) {
            let all = first.then_some(&links.inventory);
            emit(rows, quantizer, members, all, quantizers, origins, cancel)?;
        }
    }
    rows.summary.unresolved_links += links.unresolved();
    Ok(())
}

const fn eligible(quantizer: &Quantizer<'_>) -> bool {
    matches!(
        quantizer.eligibility.result,
        QuantizationEligibility::Eligible
    )
}

fn prefix(
    target: &Quantizer<'_>,
    candidates: &[Quantizer<'_>],
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<Evidence, CatalogError> {
    let mut result = Evidence::default();
    for candidate in candidates {
        check_cancel(cancel)?;
        result.append(&candidate.eligibility.admission)?;
        result.header(
            &origins.record(candidate.record_index),
            RecordHeaderInput::Field(RecordHeaderField::Guid),
            true,
        )?;
        if candidate.guid != target.guid {
            continue;
        }
        result.append(&candidate.eligibility.evidence)?;
        if eligible(candidate) {
            break;
        }
    }
    if let Some(owner) = target.owner {
        result.header(
            &origins.record(owner.record_index),
            RecordHeaderInput::Field(RecordHeaderField::Guid),
            true,
        )?;
    }
    Ok(result)
}

fn emit(
    rows: &mut QualityRows<'_>,
    quantizer: &Quantizer<'_>,
    members: Members,
    all: Option<&Evidence>,
    candidates: &[Quantizer<'_>],
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    let Some(owner) = quantizer.owner else {
        return Ok(());
    };
    let own = own_eligibility(quantizer)?;
    let eligible = eligible(quantizer);
    let selected = Arc::new(if eligible {
        prefix(quantizer, candidates, origins, cancel)?
    } else {
        own.copy()?
    });
    let mut aggregate = selected.copy()?;
    if let Some(all) = all {
        aggregate.append(all)?;
    }
    let cells = member_cells(
        members,
        &selected,
        &mut aggregate,
        &mut rows.fallbacks,
        origins,
        cancel,
    )?;
    if eligible {
        rows.summary.unused += usize::from(cells.is_empty());
    }
    rows.inverse.push(row(
        owner,
        quantizer.eligibility.result,
        &own,
        &aggregate,
        cells,
        origins,
    )?);
    Ok(())
}

fn row(
    owner: QualityOwner<'_>,
    eligibility: QuantizationEligibility,
    own: &Evidence,
    aggregate: &Evidence,
    members: Vec<SourcedValue<CatalogGuid>>,
    origins: &CatalogOrigins,
) -> Result<QualityQuantizationResources, CatalogError> {
    let context = origins.record(owner.record_index);
    Ok(QualityQuantizationResources {
        physical_guid: owner.guid,
        physical_index: owner.record_index,
        guid: context.header_selector(RecordHeaderField::Guid, SourceValue::Present(owner.guid))?,
        record_index: context.header_selector(
            RecordHeaderField::RecordIndex,
            SourceValue::Present(owner.record_index),
        )?,
        eligibility: own.value(origins, eligibility),
        resource_type_guids: aggregate.value(origins, members),
    })
}

fn own_eligibility(quantizer: &Quantizer<'_>) -> Result<Evidence, CatalogError> {
    let mut evidence = quantizer.eligibility.admission.copy()?;
    evidence.append(&quantizer.eligibility.evidence)?;
    Ok(evidence)
}

fn member_cells(
    members: Members,
    selected: &Arc<Evidence>,
    aggregate: &mut Evidence,
    fallbacks: &mut Fallbacks,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<Vec<SourcedValue<CatalogGuid>>, CatalogError> {
    let mut ordered = members.into_iter().collect::<Vec<_>>();
    ordered.sort_by_cached_key(|(guid, _)| CatalogGuid(*guid).text());
    let mut cells = Vec::new();
    cells
        .try_reserve(ordered.len())
        .map_err(CatalogError::SourceUseAllocation)?;
    for (guid, member) in ordered {
        check_cancel(cancel)?;
        let mut relationship = member.decision;
        relationship.append(&member.values)?;
        aggregate.append(&member.values)?;
        let mut exact = selected.copy()?;
        exact.append(&relationship)?;
        cells.push(exact.value(origins, CatalogGuid(guid)));
        fallbacks.retain(guid, selected, relationship)?;
    }
    Ok(cells)
}
