// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashSet};

use p4kx_extraction::{CancelToken, Guid, Record, TreeBuilder};

use super::{RESOURCE_TYPE, Scan, resources_of};
use crate::commodity::resource_commodity;
use crate::contract::{Blueprint, QualityQuantization};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::picked::Picked;
use crate::scan_health::{ScanError, ScanRead, SourceTrace};
use crate::{CatalogError, Kind};

impl<'f> Scan<'_, '_, 'f> {
    /// Reads the used additional resource definitions at their actual projection boundary.
    pub(super) fn resource_commodities(
        &self,
        picked: &[Picked<'f>],
        blueprints: &[Blueprint<'f>],
        quantizations: &[QualityQuantization<'f>],
        sources: &[SourceObservation],
    ) -> Result<ScanRead<'f>, CatalogError> {
        let used = unrepresented_resources(picked, blueprints, quantizations);
        let physical: BTreeSet<_> = sources
            .iter()
            .filter(|source| {
                source.kind == Some(Kind::ResourceTypes)
                    && used.contains(&source.guid)
                    && (source.role == ExportSourceRole::SelectionCandidate
                        || (source.role == ExportSourceRole::Selected
                            && !matches!(
                                source.disposition,
                                SourceDisposition::SkippedBeforeRead
                                    | SourceDisposition::SkippedAfterRead
                            )))
            })
            .map(|source| source.record_index)
            .collect();
        let mut trees = TreeBuilder::new();
        let mut read = ScanRead {
            picked: Vec::new(),
            ledger: ExportLedger::default(),
        };
        for index in physical {
            if self.options.cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let record = self.forge.record(index).ok_or(CatalogError::Accounting(
                crate::ExportAccountingError::MissingPhysicalReferenceTarget { index },
            ))?;
            let mut trace = SourceTrace::new(&record);
            trace.candidate(Kind::Commodities);
            let mut ledger = ExportLedger::default();
            let result = self.resource(&mut trees, &record, &mut trace, &mut ledger);
            if let Some(found) = crate::scan_health::finish(&record, &trace, result, ledger)? {
                read.picked.extend(found.picked);
                read.ledger.merge(found.ledger);
            }
        }
        Ok(read)
    }

    fn resource(
        &self,
        trees: &mut TreeBuilder<'f>,
        record: &Record<'f>,
        trace: &mut SourceTrace,
        ledger: &mut ExportLedger,
    ) -> Result<Option<Picked<'f>>, ScanError> {
        if record.type_name()? != RESOURCE_TYPE {
            return Ok(None);
        }
        let path = record.record_path()?;
        if self
            .options
            .filter
            .is_some_and(|filter| !filter.matches(path))
        {
            return Ok(None);
        }
        if crate::selection::is_kind_leftover(Kind::ResourceTypes, path, record.name()?) {
            return Ok(None);
        }
        trace.selected(Kind::Commodities);
        let tree = trees.build(record)?;
        trace.projecting();
        ledger.merge(crate::export_field_walk::inspect(
            &tree,
            record,
            Some(Kind::Commodities),
            ExportSourceRole::Selected,
            self.options.cancel,
        )?);
        Ok(Some(Picked::Commodity(resource_commodity(
            &tree,
            self.texts,
            &self.groups,
        ))))
    }
}

fn unrepresented_resources(
    picked: &[Picked<'_>],
    blueprints: &[Blueprint<'_>],
    quantizations: &[QualityQuantization<'_>],
) -> HashSet<Guid> {
    let mut carried = HashSet::new();
    for pick in picked {
        if let Picked::Commodity(commodity) = pick {
            carried.extend(commodity.subtype_guid.map(|guid| guid.0));
        }
    }
    let mut used = Vec::new();
    for tier in blueprints.iter().flat_map(|blueprint| &blueprint.tiers) {
        for line in tier.mandatory.iter().chain(&tier.optional) {
            resources_of(line, &mut used);
        }
    }
    used.extend(
        quantizations
            .iter()
            .flat_map(|quantization| &quantization.resource_type_guids)
            .map(|guid| guid.0),
    );
    used.into_iter()
        .filter(|guid| !carried.contains(guid))
        .collect()
}
