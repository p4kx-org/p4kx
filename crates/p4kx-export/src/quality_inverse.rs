// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap, HashSet};

use p4kx_extraction::{CancelToken, Guid, SourceValue};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};

use super::assembly::Quantizer;
use super::contract::{QualityOwner, QualityQuantizationResources, QuantizationEligibility};
use super::evidence::Evidence;
use super::resource_links::ResourceLinks;
use super::{QualityRows, QuantizationUse, Retention};

#[derive(Default)]
struct Member {
    decision: Evidence,
    values: Evidence,
}

struct Links {
    values: Vec<Guid>,
    members: BTreeMap<Guid, Member>,
    evidence: Evidence,
}

pub(super) fn fill<'f>(
    rows: &mut QualityRows<'f>,
    quantizers: &[Quantizer<'f>],
    resources: &[ResourceLinks],
    inventory: &Evidence,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    let mut used = HashSet::new();
    let mut all_links = HashMap::<Guid, HashSet<Guid>>::new();
    for resource in resources {
        for target in &resource.targets {
            all_links
                .entry(*target)
                .or_default()
                .insert(resource.resource);
        }
    }
    for quantizer in quantizers {
        check(cancel)?;
        let eligible = matches!(
            quantizer.eligibility.result,
            QuantizationEligibility::Eligible
        );
        let first = eligible && used.insert(quantizer.guid);
        let links = if first {
            all_links.remove(&quantizer.guid);
            linked(quantizer.guid, resources, inventory, cancel)?
        } else {
            Links {
                values: Vec::new(),
                members: BTreeMap::new(),
                evidence: Evidence::default(),
            }
        };
        if matches!(quantizer.retention, Retention::Leftover) && first {
            rows.summary.unresolved_links += links.values.len();
        }
        if !matches!(quantizer.retention, Retention::Selected) {
            continue;
        }
        emit(rows, quantizer, &links, first, quantizers, origins, cancel)?;
    }
    rows.summary.unresolved_links += all_links.values().map(HashSet::len).sum::<usize>();
    Ok(())
}

fn linked(
    guid: Guid,
    resources: &[ResourceLinks],
    inventory: &Evidence,
    cancel: Option<&CancelToken>,
) -> Result<Links, CatalogError> {
    let mut evidence = inventory.clone();
    let mut values = Vec::new();
    let mut members = BTreeMap::<Guid, Member>::new();
    for resource in resources {
        check(cancel)?;
        evidence.append(&resource.evidence);
        if resource.targets.contains(&guid) {
            values.push(resource.resource);
            evidence.keep(resource.guid_source.clone());
            let member = members.entry(resource.resource).or_default();
            member.decision.append(&resource.evidence);
            member.values.keep(resource.guid_source.clone());
        }
    }
    values.sort_unstable();
    values.dedup();
    values.sort_by_cached_key(|guid| CatalogGuid(*guid).text());
    Ok(Links {
        values,
        members,
        evidence,
    })
}

fn prefix(
    target: &Quantizer<'_>,
    candidates: &[Quantizer<'_>],
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<Evidence, CatalogError> {
    let mut result = Evidence::default();
    for candidate in candidates {
        check(cancel)?;
        result.append(&candidate.eligibility.admission);
        result.header(
            &origins.record(candidate.record_index),
            RecordHeaderInput::Field(RecordHeaderField::Guid),
            true,
        )?;
        if candidate.guid != target.guid {
            continue;
        }
        result.append(&candidate.eligibility.evidence);
        if matches!(
            candidate.eligibility.result,
            QuantizationEligibility::Eligible
        ) {
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

fn emit<'f>(
    rows: &mut QualityRows<'f>,
    quantizer: &Quantizer<'f>,
    links: &Links,
    first: bool,
    candidates: &[Quantizer<'f>],
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    let Some(owner) = quantizer.owner else {
        return Ok(());
    };
    let context = origins.record(owner.record_index);
    let eligible = matches!(
        quantizer.eligibility.result,
        QuantizationEligibility::Eligible
    );
    let mut evidence = if eligible {
        prefix(quantizer, candidates, origins, cancel)?
    } else {
        own_eligibility(quantizer)
    };
    let selected = evidence.clone();
    if first {
        evidence.append(&links.evidence);
    }
    let members = members(links, &selected, origins, cancel)?;
    if eligible {
        eligible_use(rows, owner, &links.values);
    }
    rows.inverse.push(QualityQuantizationResources {
        physical_guid: owner.guid,
        physical_index: owner.record_index,
        inputs: evidence.sources(origins)?,
        complete: evidence.complete,
        guid: context.header_selector(RecordHeaderField::Guid, SourceValue::Present(owner.guid))?,
        record_index: context.header_selector(
            RecordHeaderField::RecordIndex,
            SourceValue::Present(owner.record_index),
        )?,
        eligibility: own_eligibility(quantizer).value(origins, quantizer.eligibility.result)?,
        resource_type_guids: evidence.value(origins, members)?,
    });
    Ok(())
}

fn own_eligibility(quantizer: &Quantizer<'_>) -> Evidence {
    let mut evidence = quantizer.eligibility.admission.clone();
    evidence.append(&quantizer.eligibility.evidence);
    evidence
}

fn eligible_use<'f>(rows: &mut QualityRows<'f>, owner: QualityOwner<'f>, values: &[Guid]) {
    rows.summary.unused += usize::from(values.is_empty());
    rows.uses.push(QuantizationUse {
        guid: owner.guid,
        record_index: owner.record_index,
        resource_type_guids: values.iter().copied().map(CatalogGuid).collect(),
    });
}

fn members(
    links: &Links,
    decision: &Evidence,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<Vec<p4kx_extraction::SourcedValue<CatalogGuid>>, CatalogError> {
    links
        .values
        .iter()
        .map(|guid| {
            check(cancel)?;
            let mut evidence = decision.clone();
            let member = links
                .members
                .get(guid)
                .ok_or(crate::ExportAccountingError::MissingFieldInspection)?;
            evidence.append(&member.decision);
            let mut uses = evidence.sources(origins)?;
            uses.extend(member.values.sources(origins)?);
            Ok(origins.uses.value(
                SourceValue::Present(CatalogGuid(*guid)),
                &uses,
                evidence.complete && member.values.complete,
            ))
        })
        .collect()
}

fn check(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        Err(CatalogError::Cancelled)
    } else {
        Ok(())
    }
}
