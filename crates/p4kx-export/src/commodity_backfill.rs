// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{Guid, RecordTree, SourceUseRole, SourceValue, SourcedValue};

use crate::CatalogError;
use crate::catalog_crafting::CraftingKinds;
use crate::catalog_origins::CatalogOrigins;
use crate::commodity_backfill_input::{CommodityBackfillInput, CommodityInputLimit};
use crate::commodity_contract::{CommoditySelection, CommoditySelectionDiagnostic};
use crate::commodity_inputs::CommodityInputs;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::picked::Picked;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::selection::Kind;

pub(crate) struct BackfillEvidence<'context> {
    pub(crate) inputs: CommodityInputs<'context>,
    pub(crate) diagnostics: Vec<CommoditySelectionDiagnostic>,
}

/// Retains actual eligibility and representation inputs only on the backfill-presence decision.
pub(crate) fn for_resource<'context>(
    tree: &RecordTree<'_>,
    picked: &[Picked<'_>],
    crafting: &CraftingKinds<'_>,
    scanned: &ExportLedger,
    origins: &'context CatalogOrigins,
) -> Result<BackfillEvidence<'context>, CatalogError> {
    let mut inputs = CommodityInputs::new(origins);
    for field in [
        RecordHeaderInput::Field(RecordHeaderField::Guid),
        RecordHeaderInput::Field(RecordHeaderField::SourceType),
        RecordHeaderInput::NormalizedPath,
        RecordHeaderInput::Field(RecordHeaderField::RecordName),
    ] {
        inputs.header_input(tree.record_index(), field, SourceUseRole::Selector)?;
    }
    let mut eligibility = false;
    for input in &crafting.ledger.commodity_backfill_inputs {
        if input.resource == tree.guid() && active(input, crafting) {
            eligibility = true;
            observe(input, &mut inputs)?;
            if let Some(quantization) = input.quantization {
                quantization_selectors(quantization, crafting, &mut inputs)?;
            }
        }
    }
    for pick in picked {
        representation(pick, &mut inputs);
    }
    inputs.require_complete(scanned.sources.iter().all(|source| {
        !source.disposition.failed()
            || !(source.kind == Some(Kind::Commodities)
                || (source.kind.is_none() && source.role == ExportSourceRole::SelectionCandidate))
    }));
    let diagnostics = limits(tree.guid(), crafting, origins, &mut inputs)?;
    if !eligibility && diagnostics.is_empty() {
        return Err(CatalogError::Accounting(
            crate::ExportAccountingError::MissingFieldInspection,
        ));
    }
    Ok(BackfillEvidence {
        inputs,
        diagnostics,
    })
}

fn representation(pick: &Picked<'_>, inputs: &mut CommodityInputs<'_>) {
    match pick {
        Picked::Commodity(value) => {
            inputs.extend(value.relationship_inputs.iter().cloned(), true);
            inputs.extend(value.crafter_inputs.iter().cloned(), value.crafter_complete);
        }
        Picked::Crafted(inner, _) => {
            if let Picked::Commodity(value) = inner.as_ref() {
                inputs.extend(value.crafter_inputs.iter().cloned(), value.crafter_complete);
            }
        }
        _ => {}
    }
}
fn active(input: &CommodityBackfillInput, crafting: &CraftingKinds<'_>) -> bool {
    used_target(
        input.resource,
        input.quantization,
        input.record_index,
        crafting,
    )
}

fn used_target(
    resource: Guid,
    quantization: Option<Guid>,
    record_index: u32,
    crafting: &CraftingKinds<'_>,
) -> bool {
    quantization.map_or_else(
        || {
            crafting.ledger.sources.iter().any(|source| {
                source.record_index == record_index && blueprint_source(source, crafting)
            })
        },
        |guid| {
            crafting.quantizations.iter().any(|row| {
                row.guid.0 == guid
                    && row
                        .resource_type_guids
                        .iter()
                        .any(|guid| guid.0 == resource)
            })
        },
    )
}

fn blueprint_source(source: &SourceObservation, crafting: &CraftingKinds<'_>) -> bool {
    source.kind == Some(Kind::Blueprints)
        && source.disposition == SourceDisposition::Projected
        && crafting.blueprints.iter().any(|row| {
            row.guid.0 == source.guid && source.record_path.as_deref() == Some(row.record_path)
        })
}

fn observe(
    input: &CommodityBackfillInput,
    inputs: &mut CommodityInputs<'_>,
) -> Result<(), CatalogError> {
    for field in [
        RecordHeaderInput::Field(RecordHeaderField::SourceType),
        RecordHeaderInput::NormalizedPath,
        RecordHeaderInput::Field(RecordHeaderField::RecordName),
    ] {
        inputs.header_input(input.record_index, field, SourceUseRole::Selector)?;
    }
    for field in &input.selectors {
        inputs.at(input.record_index, field, SourceUseRole::Selector)?;
    }
    inputs.at(input.record_index, &input.field, SourceUseRole::Selector)
}

fn quantization_selectors(
    guid: Guid,
    crafting: &CraftingKinds<'_>,
    inputs: &mut CommodityInputs<'_>,
) -> Result<(), CatalogError> {
    let mut observed = false;
    for source in &crafting.ledger.sources {
        if source.kind != Some(Kind::QualityQuantizations)
            || source.guid != guid
            || source.disposition != SourceDisposition::Projected
        {
            continue;
        }
        if !crafting.quantizations.iter().any(|row| {
            row.guid.0 == source.guid && source.record_path.as_deref() == Some(row.record_path)
        }) {
            continue;
        }
        for field in [
            RecordHeaderInput::Field(RecordHeaderField::Guid),
            RecordHeaderInput::Field(RecordHeaderField::SourceType),
            RecordHeaderInput::NormalizedPath,
            RecordHeaderInput::Field(RecordHeaderField::RecordName),
        ] {
            inputs.header_input(source.record_index, field, SourceUseRole::Selector)?;
        }
        observed = true;
    }
    inputs.require_complete(observed);
    Ok(())
}

fn limit_applies(
    limit: &CommodityInputLimit,
    resource: Guid,
    crafting: &CraftingKinds<'_>,
) -> bool {
    if let Some(target) = limit.target {
        return target.resource == resource
            && used_target(resource, target.quantization, limit.record_index, crafting);
    }
    crafting.ledger.sources.iter().any(|source| {
        source.record_index == limit.record_index
            && ((blueprint_source(source, crafting) && blueprint_uses(source, resource, crafting))
                || (source.guid == resource
                    && crafting.quantizations.iter().any(|row| {
                        row.resource_type_guids
                            .iter()
                            .any(|guid| guid.0 == resource)
                    })))
    })
}

fn blueprint_uses(
    source: &SourceObservation,
    resource: Guid,
    crafting: &CraftingKinds<'_>,
) -> bool {
    crafting
        .blueprints
        .iter()
        .filter(|row| {
            row.guid.0 == source.guid && source.record_path.as_deref() == Some(row.record_path)
        })
        .any(|row| {
            row.tiers.iter().any(|tier| {
                tier.mandatory
                    .iter()
                    .chain(&tier.optional)
                    .any(|line| crate::cost_resources::contains(line, resource))
            })
        })
}
fn limits(
    resource: Guid,
    crafting: &CraftingKinds<'_>,
    origins: &CatalogOrigins,
    inputs: &mut CommodityInputs<'_>,
) -> Result<Vec<CommoditySelectionDiagnostic>, CatalogError> {
    let mut diagnostics = Vec::new();
    let mut reported = BTreeSet::new();
    for limit in &crafting.ledger.commodity_input_limits {
        if !limit_applies(limit, resource, crafting) {
            continue;
        }
        inputs.require_complete(false);
        if !reported.insert((limit.record_index, limit.target.is_some())) {
            continue;
        }
        let record_index = origins.record(limit.record_index).header_selector(
            RecordHeaderField::RecordIndex,
            SourceValue::Present(limit.record_index),
        )?;
        diagnostics.push(CommoditySelectionDiagnostic {
            code: if limit.target.is_some() {
                "originMetadataLimit"
            } else {
                "originTargetInventoryLimit"
            },
            record_index,
        });
    }
    Ok(diagnostics)
}

/// Materializes the presence decision after its actual contributor set is prepared.
pub(crate) fn selection(evidence: &BackfillEvidence<'_>) -> SourcedValue<CommoditySelection> {
    evidence.inputs.value(CommoditySelection::ResourceFallback)
}
