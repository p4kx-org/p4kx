// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{
    Guid, Record, RecordTree, SharedFieldOrigin, SourceUse, SourceUseRole, SourceValue,
    SourcedValue,
};

use crate::CatalogError;
use crate::blueprint_backfill_source::BlueprintResources;
use crate::catalog_origins::CatalogOrigins;
use crate::commodity_contract::{Commodity, CommoditySelection};
use crate::commodity_inputs::CommodityInputs;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::picked::Picked;
use crate::quality_catalog::Fallbacks;
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
                self.observe_entity(row, false);
            }
            Picked::Crafted(inner, _) => {
                if let Picked::Commodity(row) = inner.as_mut() {
                    self.observe_entity(row, true);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn represents(&self, guid: &Guid) -> bool {
        self.represented.contains(guid)
    }

    pub(crate) fn observe_entity(&mut self, row: &mut Commodity<'_>, crafter: bool) {
        self.observe_source(
            row.subtype_guid.map(|guid| guid.0),
            std::mem::take(&mut row.relationship_inputs),
            std::mem::take(&mut row.crafter_inputs),
            row.crafter_complete,
            crafter,
        );
    }

    pub(crate) fn observe_source(
        &mut self,
        resource: Option<Guid>,
        links: Vec<SourceUse>,
        crafter_inputs: Vec<SourceUse>,
        complete: bool,
        crafter: bool,
    ) {
        if !crafter {
            self.represented.extend(resource);
            self.inputs.extend(links);
        }
        self.inputs.extend(crafter_inputs);
        self.complete &= complete;
    }

    pub(crate) fn into_origin(
        self,
        scanned: &ExportLedger,
        origins: &CatalogOrigins,
    ) -> SharedFieldOrigin {
        let complete = scanned.sources.iter().all(|source| {
            !source.disposition.failed() || !failure_affects_presence(source.kind, source.role)
        });
        self.finish(complete, origins)
    }

    pub(crate) fn finish(self, complete: bool, origins: &CatalogOrigins) -> SharedFieldOrigin {
        origins
            .uses
            .shared_origin(&self.inputs, complete && self.complete)
    }
}

/// Returns whether the failed header could hide an admitted Commodity.
pub(crate) fn header_failure_affects_presence(record: &Record<'_>) -> bool {
    let kind = crate::scan_health::SourceTrace::new(record).selection_candidate_kind();
    failure_affects_presence(kind, ExportSourceRole::SelectionCandidate)
}

fn failure_affects_presence(kind: Option<Kind>, role: ExportSourceRole) -> bool {
    kind == Some(Kind::Commodities)
        || (kind.is_none() && role == ExportSourceRole::SelectionCandidate)
}

/// Retains actual eligibility and representation inputs only on the backfill-presence decision.
pub(crate) fn for_resource<'evidence>(
    tree: &RecordTree<'_>,
    representation: &SharedFieldOrigin,
    blueprints: impl IntoIterator<Item = &'evidence BlueprintResources>,
    quality: &Fallbacks,
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
        eligibility |= blueprint.observe(tree.guid(), &mut inputs)?;
    }
    if let Some(members) = quality.evidence(tree.guid()) {
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
