// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{
    Guid, RecordTree, SharedFieldOrigin, SourceUse, SourceUseRole, SourceValue, SourcedValue,
};

use crate::CatalogError;
use crate::blueprint_catalog_rows::BlueprintRow;
use crate::catalog_origins::CatalogOrigins;
use crate::commodity_contract::{Commodity, CommoditySelection};
use crate::commodity_inputs::CommodityInputs;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::picked::Picked;
use crate::quality_catalog::QualityRows;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::selection::Kind;

/// Original Commodity observations needed to decide `ResourceType` backfill.
pub(crate) struct CommodityPresence {
    represented: HashSet<Guid>,
    inputs: Vec<SourceUse>,
    complete: bool,
}

impl CommodityPresence {
    pub(crate) fn new() -> Self {
        Self {
            represented: HashSet::new(),
            inputs: Vec::new(),
            complete: true,
        }
    }

    pub(crate) fn observe(&mut self, pick: &mut Picked<'_>) {
        match pick {
            Picked::Commodity(row) => {
                self.represented.extend(row.subtype_guid.map(|guid| guid.0));
                self.inputs
                    .extend(std::mem::take(&mut row.relationship_inputs));
                self.crafter(row);
            }
            Picked::Crafted(inner, _) => {
                if let Picked::Commodity(row) = inner.as_mut() {
                    drop(std::mem::take(&mut row.relationship_inputs));
                    self.crafter(row);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn represents(&self, guid: &Guid) -> bool {
        self.represented.contains(guid)
    }

    pub(crate) fn into_origin(
        self,
        scanned: &ExportLedger,
        origins: &CatalogOrigins,
    ) -> SharedFieldOrigin {
        let complete = self.complete
            && scanned.sources.iter().all(|source| {
                !source.disposition.failed()
                    || !(source.kind == Some(Kind::Commodities)
                        || (source.kind.is_none()
                            && source.role == ExportSourceRole::SelectionCandidate))
            });
        origins.uses.shared_origin(&self.inputs, complete)
    }

    fn crafter(&mut self, row: &mut Commodity<'_>) {
        self.inputs.extend(std::mem::take(&mut row.crafter_inputs));
        self.complete &= row.crafter_complete;
    }
}

/// Retains actual eligibility and representation inputs only on the backfill-presence decision.
pub(crate) fn for_resource(
    tree: &RecordTree<'_>,
    representation: &SharedFieldOrigin,
    blueprints: &[BlueprintRow<'_>],
    quality: &QualityRows<'_>,
    origins: &CatalogOrigins,
) -> Result<SourcedValue<CommoditySelection>, CatalogError> {
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
    for blueprint in blueprints {
        eligibility |= blueprint.resources.observe(tree.guid(), &mut inputs)?;
    }
    if let Some(members) = quality.fallbacks.evidence(tree.guid()) {
        for original in members {
            inputs.extend_retained(original.inputs(), original.complete)?;
        }
        eligibility = true;
    }

    if !eligibility {
        return Err(CatalogError::Accounting(
            crate::ExportAccountingError::MissingFieldInspection,
        ));
    }
    let (local, complete) = inputs.into_evidence();
    Ok(origins.uses.value_with_shared_origin(
        SourceValue::Present(CommoditySelection::ResourceFallback),
        representation,
        &local,
        complete,
    )?)
}
