// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceUseRole};

use crate::CatalogError;
use crate::catalog_crafting::CraftingKinds;
use crate::catalog_origins::CatalogOrigins;
use crate::commodity_inputs::CommodityInputs;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::picked::Picked;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::selection::Kind;

/// Retains actual eligibility and representation inputs only on the backfill-presence decision.
pub(crate) fn for_resource<'context>(
    tree: &RecordTree<'_>,
    picked: &[Picked<'_>],
    crafting: &CraftingKinds<'_>,
    scanned: &ExportLedger,
    origins: &'context CatalogOrigins,
) -> Result<CommodityInputs<'context>, CatalogError> {
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
    for blueprint in &crafting.blueprints {
        eligibility |= blueprint.resources.observe(tree.guid(), &mut inputs)?;
    }
    if let Some(members) = crafting.quality.fallbacks.evidence(tree.guid()) {
        for original in members {
            inputs.extend_retained(original.inputs(), original.complete)?;
        }
        eligibility = true;
    }
    for pick in picked {
        representation(pick, &mut inputs);
    }
    inputs.require_complete(scanned.sources.iter().all(|source| {
        !source.disposition.failed()
            || !(source.kind == Some(Kind::Commodities)
                || (source.kind.is_none() && source.role == ExportSourceRole::SelectionCandidate))
    }));
    if !eligibility {
        return Err(CatalogError::Accounting(
            crate::ExportAccountingError::MissingFieldInspection,
        ));
    }
    Ok(inputs)
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
