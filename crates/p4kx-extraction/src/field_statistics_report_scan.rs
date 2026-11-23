// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::sync::Mutex;

use p4kx_dataforge::query::{QueryRecordAdmission, QuerySource, QueryTypeSource, QueryUnavailable};
use p4kx_dataforge::{
    DataForge, FieldStatisticsObservation, FieldStatisticsReduction, LayoutSource,
    RecordPathObservation,
};

use crate::dataforge_layout_evidence;
use crate::field_statistics_evidence::FieldStatisticsSourceUses;
use crate::query_admission_evidence::{self, HeaderCensus};
use crate::query_source_evidence::QuerySourceSite;
use crate::{
    FieldOrigin, SourceEvidenceError, SourceScope, SourceUseRef, SourceUseRole, SourceUses,
};

use super::counts::{FieldStatisticsCounterProof, ScanAccounting};
use super::instance_bounds::InstanceBounds;
use super::rows::{FieldStatisticsRowProof, Rows, Uses};
use super::{FieldStatisticsReportError, FieldStatisticsSelectionProof};

pub(super) struct BoundCapture {
    pub(super) selection_origin: FieldOrigin,
    pub(super) selection_attempts: Vec<SourceUseRef>,
    pub(super) unresolved_ancestry_positions: Vec<usize>,
    pub(super) unavailable_traversals: Vec<QueryUnavailable>,
    pub(super) fields: Vec<FieldStatisticsRowProof>,
    pub(super) counters: FieldStatisticsCounterProof,
    pub(super) root_attempts: Vec<SourceUseRef>,
}

#[derive(Default)]
pub(super) struct Capture {
    selection: Uses,
    headers: HeaderCensus,
    query_candidates: usize,
    selection_attempts: Uses,
    ancestry_gaps: BTreeSet<usize>,
    instance_bounds: InstanceBounds,
    unavailable_traversals: BTreeSet<QueryUnavailable>,
    rows: Rows,
    accounting: ScanAccounting,
    error: Option<SourceEvidenceError>,
}

pub(super) trait Observe {
    fn path(&self, scope: &SourceScope, entry: &RecordPathObservation);
    fn query_type(&self, scope: &SourceScope, input: &QueryTypeSource);
    fn query_layout(&self, scope: &SourceScope, input: &LayoutSource);
    fn query_source(&self, scope: &SourceScope, input: QuerySource);
    fn query_record(&self, scope: &SourceScope, entry: &QueryRecordAdmission);
    fn field(&self, scope: &SourceScope, event: &FieldStatisticsObservation<'_>);
}

impl Observe for Mutex<Capture> {
    fn path(&self, scope: &SourceScope, entry: &RecordPathObservation) {
        collect(self, |capture| {
            capture.observe_header(scope, |headers| headers.all_records(entry))
        });
    }

    fn query_type(&self, scope: &SourceScope, input: &QueryTypeSource) {
        collect(self, |capture| {
            if let QueryTypeSource::UnreadableAncestry { index, .. } = input {
                capture.ancestry_gaps.insert(*index);
            }
            capture.select(scope, query_admission_evidence::type_sites(input))
        });
    }

    fn query_layout(&self, scope: &SourceScope, input: &LayoutSource) {
        collect(self, |capture| {
            if let LayoutSource::UnavailableAncestry { index, .. } = input {
                capture.ancestry_gaps.insert(*index);
            }
            capture.select(scope, dataforge_layout_evidence::sites(input))
        });
    }

    fn query_source(&self, scope: &SourceScope, input: QuerySource) {
        collect(self, |capture| {
            match input {
                QuerySource::AncestorGap { index } => {
                    capture.ancestry_gaps.insert(index);
                }
                QuerySource::InstanceBlockBound { structure } => {
                    capture.instance_bounds.count(structure);
                }
                QuerySource::InstanceExtent {
                    structure,
                    resolved,
                } => {
                    capture.instance_bounds.extent(structure, resolved);
                }
                QuerySource::Unavailable(gap) => {
                    capture.unavailable_traversals.insert(gap);
                }
                _ => {}
            }
            let site = QuerySourceSite::of(input, SourceUseRole::Selector);
            let site = site.map(|mut site| {
                if matches!(input, QuerySource::ReferenceGap { .. }) {
                    site.role = SourceUseRole::Attempted;
                }
                site
            });
            capture.select(scope, site)
        });
    }

    fn query_record(&self, scope: &SourceScope, entry: &QueryRecordAdmission) {
        collect(self, |capture| {
            capture.query_candidates += 1;

            capture.observe_header(scope, |headers| headers.record(entry))
        });
    }

    fn field(&self, scope: &SourceScope, event: &FieldStatisticsObservation<'_>) {
        collect(self, |capture| {
            let bound = FieldStatisticsSourceUses::bind(scope, event)?;
            capture.accounting.observe(event);
            capture.rows.observe(event, &bound);
            Ok(())
        });
    }
}

fn collect(
    capture: &Mutex<Capture>,
    observe: impl FnOnce(&mut Capture) -> Result<(), SourceEvidenceError>,
) {
    if let Ok(mut capture) = capture.lock()
        && capture.error.is_none()
        && let Err(error) = observe(&mut capture)
    {
        capture.error = Some(error);
    }
}

impl Capture {
    pub(super) fn selection_complete(&self, proof: FieldStatisticsSelectionProof) -> bool {
        match proof {
            FieldStatisticsSelectionProof::AllRecords => true,
            FieldStatisticsSelectionProof::QueryHeaders { record_count } => {
                self.query_candidates == record_count
                    && self.selection_attempts.iter().next().is_none()
                    && self.ancestry_gaps.is_empty()
            }
            _ => false,
        }
    }

    fn observe_header(
        &mut self,
        scope: &SourceScope,
        observe: impl FnOnce(&mut HeaderCensus) -> Option<QuerySourceSite>,
    ) -> Result<(), SourceEvidenceError> {
        let attempt = observe(&mut self.headers);
        self.select(scope, attempt)
    }

    fn select(
        &mut self,
        scope: &SourceScope,
        sites: impl IntoIterator<Item = QuerySourceSite>,
    ) -> Result<(), SourceEvidenceError> {
        for site in sites {
            let source = site.bind(scope)?;
            if source.role().contributes() {
                self.selection.insert(&source);
            } else {
                self.selection_attempts.insert(&source);
            }
        }
        Ok(())
    }

    pub(super) fn finish(
        mut self,
        inventory: &SourceUses,
        scope: &SourceScope,
        forge: &DataForge<'_>,
        reduction: &FieldStatisticsReduction<'_>,
        selection_complete: bool,
    ) -> Result<BoundCapture, FieldStatisticsReportError> {
        if let Some(error) = self.error {
            return Err(error.into());
        }
        self.select(
            scope,
            ["recordCount", "structCount"]
                .map(|field| QuerySourceSite::header(field, SourceUseRole::Selector)),
        )?;
        let headers = std::mem::take(&mut self.headers);
        self.select(scope, headers.sites())?;
        let bounds = std::mem::take(&mut self.instance_bounds).sites(forge)?;
        self.select(scope, bounds)?;
        for source in self.rows.root_uses() {
            self.selection.insert(source);
        }
        let selection =
            inventory.shared_origin_from_uses(self.selection.iter(), selection_complete)?;
        let mut fields = Vec::with_capacity(reduction.statistics().fields().len());
        for ordinal in 0..reduction.statistics().fields().len() {
            let row = reduction
                .row(ordinal)
                .ok_or(FieldStatisticsReportError::UnboundField)?;
            let proof = self
                .rows
                .bind(inventory, scope, &row, ordinal, &selection)?
                .ok_or(FieldStatisticsReportError::UnboundField)?;
            fields.push(proof);
        }
        let counters =
            self.accounting
                .bind(inventory, reduction.statistics(), &selection, &self.rows)?;
        Ok(BoundCapture {
            selection_origin: counters.records.clone(),
            counters,
            root_attempts: self
                .rows
                .root_attempts()
                .map(|source| source.reference().clone())
                .collect(),
            selection_attempts: self.selection_attempts.refs(),
            unresolved_ancestry_positions: self.ancestry_gaps.into_iter().collect(),
            unavailable_traversals: self.unavailable_traversals.into_iter().collect(),
            fields,
        })
    }
}
