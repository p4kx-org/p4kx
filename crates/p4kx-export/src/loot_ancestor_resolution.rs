// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, btree_map};

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Filter, Guid, Record, SourceUseRef, SourceUseRole,
    SourcedValue, TreeBuilder,
};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::{CatalogRecordCoverage, starmap_name_owners};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::starmap_object_definition;
use crate::guid_text::CatalogGuid;
use crate::harvest_locations::{LocationObject, Objects};
use crate::record_header_field::RecordHeaderInput;
use crate::record_observation::RecordObservation;
use crate::starmap::ObjectPlacement;
use crate::starmap_containers::Placements;
use crate::starmap_containers::proof::PlacementEvidence;
use crate::{CatalogError, ExportAccountingError};

/// Observed owner selection for one demanded GUID in original Record order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AncestorOwner {
    NoAncestor,
    NotAdmitted,
    Unique(u32),
    Ambiguous(u32),
    Unavailable(Option<u32>),
}

pub(super) struct AncestorFacts {
    candidates: BTreeMap<Guid, Aliases>,
    pub(super) coverage: CatalogRecordCoverage,
    pub(super) failed_source_uses: Vec<SourceUseRef>,
}

struct Aliases {
    owners: Vec<PhysicalMarker>,
    above: Option<Guid>,
    placement: Option<PlacementEvidence>,
    complete: bool,
}

/// One readable physical alias and the original marker it contributed.
pub(super) struct PhysicalMarker {
    pub(super) index: u32,
    pub(super) marker: SourcedValue<bool>,
}

impl AncestorFacts {
    /// Resolves only demanded GUIDs and the original placed parents of their effective markers.
    pub(super) fn read<'source>(
        forge: &'source DataForge<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        demands: impl IntoIterator<Item = Guid>,
        placements: &Placements,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        input.check_forge(forge)?;
        let mut facts = Self {
            candidates: BTreeMap::new(),
            coverage: CatalogRecordCoverage::default(),
            failed_source_uses: Vec::new(),
        };
        let mut pending: Vec<Guid> = demands.into_iter().collect();
        let mut trees = TreeBuilder::new();
        while let Some(guid) = pending.pop() {
            checkpoint(cancel)?;
            let btree_map::Entry::Vacant(entry) = facts.candidates.entry(guid) else {
                continue;
            };
            let aliases = read_aliases(
                forge,
                input,
                guid,
                &mut trees,
                (filter, cancel),
                &mut facts.coverage,
                &mut facts.failed_source_uses,
            )?;
            let ObjectPlacement {
                placed_under_guid,
                proof,
                ..
            } = ObjectPlacement::select(CatalogGuid(guid), placements);
            let above = placed_under_guid.map(CatalogGuid::get);
            if aliases.owners.last().is_some_and(PhysicalMarker::is_marker) {
                pending.extend(above);
            }
            entry.insert(Aliases {
                above,
                placement: proof,
                ..aliases
            });
        }
        checkpoint(cancel)?;
        Ok(facts)
    }

    pub(super) fn admitted_aliases(&self, guid: Guid) -> &[PhysicalMarker] {
        self.candidates
            .get(&guid)
            .map_or(&[], |aliases| aliases.owners.as_slice())
    }

    pub(super) fn owner(&self, guid: Option<Guid>) -> AncestorOwner {
        let Some(guid) = guid else {
            return AncestorOwner::NoAncestor;
        };
        let Some(aliases) = self.candidates.get(&guid) else {
            return AncestorOwner::Unavailable(None);
        };
        let representative = aliases.owners.last().map(|owner| owner.index);
        match (aliases.complete, representative, aliases.owners.len()) {
            (false, _, _) => AncestorOwner::Unavailable(representative),
            (true, None, _) => AncestorOwner::NotAdmitted,
            (true, Some(owner), 1) => AncestorOwner::Unique(owner),
            (true, Some(owner), _) => AncestorOwner::Ambiguous(owner),
        }
    }

    pub(super) fn objects(&self) -> Objects<'_> {
        Objects::from_location_facts(self.candidates.iter().flat_map(|(&guid, aliases)| {
            aliases.owners.iter().map(move |owner| {
                let fact = LocationObject::new(
                    owner.is_marker(),
                    aliases.above,
                    Some(owner.marker.origin()),
                    aliases.placement.as_ref(),
                );
                let fact = if aliases.complete {
                    fact
                } else {
                    fact.with_incomplete_membership()
                };
                (guid, fact)
            })
        }))
    }
}

impl PhysicalMarker {
    fn is_marker(&self) -> bool {
        self.marker.value() == Some(&true)
    }
}

fn read_aliases<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    guid: Guid,
    trees: &mut TreeBuilder<'source>,
    (filter, cancel): (Option<&Filter>, Option<&CancelToken>),
    coverage: &mut CatalogRecordCoverage,
    failed_source_uses: &mut Vec<SourceUseRef>,
) -> Result<Aliases, CatalogError> {
    let first_failure = failures(coverage);
    let mut owners = Vec::new();
    for record in forge.records_by_guid(guid) {
        checkpoint(cancel)?;
        let observed = observe_alias(
            input,
            &record,
            trees,
            (filter, cancel),
            coverage,
            failed_source_uses,
        )?;
        let Some(marker) = observed else {
            continue;
        };
        let index = record.index();
        if !marker.origin().source_set_complete() {
            coverage.incomplete_fields.push(index);
        }
        if owners
            .last()
            .is_some_and(|previous: &PhysicalMarker| previous.index >= index)
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        owners.push(PhysicalMarker { index, marker });
    }
    Ok(Aliases {
        owners,
        above: None,
        placement: None,
        complete: failures(coverage) == first_failure,
    })
}

fn observe_alias<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    record: &Record<'source>,
    trees: &mut TreeBuilder<'source>,
    (filter, cancel): (Option<&Filter>, Option<&CancelToken>),
    coverage: &mut CatalogRecordCoverage,
    failed_source_uses: &mut Vec<SourceUseRef>,
) -> Result<Option<SourcedValue<bool>>, CatalogError> {
    let headers_before = coverage.unreadable_headers.len();
    let mut failed_header = None;
    let mut observed = None;
    let outcome = starmap_name_owners::visit_admitted_record(
        trees,
        record,
        filter,
        coverage,
        |field, role| {
            if role == SourceUseRole::Attempted {
                failed_header = Some(field);
            }
        },
        |record, tree| {
            let admission = RecordObservation::capture(input, record)?;
            observed = Some(starmap_object_definition::read_location_marker(
                tree, input, &admission, cancel,
            )?);
            Ok(())
        },
    );
    match outcome {
        Ok(_) => {}
        Err(CatalogError::DataForge(DataForgeError::ObservationStopped)) => {
            return Err(CatalogError::Cancelled);
        }
        Err(CatalogError::DataForge(_)) => {
            coverage.unreadable_bodies.push(record.index());
        }
        Err(error) => return Err(error),
    }
    if coverage.unreadable_headers.len() != headers_before {
        record_failed_header(input, record.index(), failed_header, failed_source_uses)?;
    }
    Ok(observed)
}

fn record_failed_header(
    input: &OriginalCatalogInput<'_, '_>,
    index: u32,
    field: Option<RecordHeaderInput>,
    failed: &mut Vec<SourceUseRef>,
) -> Result<(), CatalogError> {
    let field = field.ok_or(ExportAccountingError::MissingFieldInspection)?;
    let mut attempted = Vec::new();
    input.origins().record(index).append_header_input(
        field,
        SourceUseRole::Attempted,
        &mut attempted,
    )?;
    let [original] = attempted.as_slice() else {
        return Err(ExportAccountingError::MissingFieldInspection.into());
    };
    failed.push(original.reference().clone());
    Ok(())
}

const fn failures(coverage: &CatalogRecordCoverage) -> [usize; 3] {
    [
        coverage.unreadable_headers.len(),
        coverage.unreadable_bodies.len(),
        coverage.incomplete_fields.len(),
    ]
}

#[cfg(test)]
#[path = "loot_ancestor_resolution_tests.rs"]
mod tests;
