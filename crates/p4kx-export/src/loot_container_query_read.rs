// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, CancelToken, DataForge, FieldOrigin, Guid, SourceUseRef, SourceValueState,
};

use super::ancestor_resolution::{AncestorFacts, AncestorOwner};
use super::selection::{ContainerKey, SelectedPlacement};
use super::{ParentOrigins, PlacementValues, SystemOrigins, read_tags, row};
use crate::build::original_input::OriginalCatalogInput;

use crate::catalog_queries::loot::{
    LootContainerOccurrence, LootContainerQuery, LootContainerSnapshot, LootContainerStatus,
};
use crate::catalog_queries::{CatalogRecordCoverage, starmap_name_owners};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::EntryCapture;
use crate::contract_loot::LootContainer;
use crate::guid_text::CatalogGuid;
use crate::harvest_locations::Objects;
use crate::loot_entxml::documents::LootEntityContainerDocument;
use crate::loot_walk::{self, DocumentVisit};
use crate::starmap_containers::Placements;
use crate::starmap_index::Index;
use crate::{CatalogError, ExportAccountingError};

#[path = "loot_container_lookup.rs"]
mod lookup;

#[path = "loot_requested_ancestor.rs"]
mod requested_ancestor;

use requested_ancestor::RequestedOwner;

type DocumentCallback = fn(usize, LootEntityContainerDocument) -> Result<(), CatalogError>;

struct Outcome {
    status: LootContainerStatus,
    row: Option<LootContainer>,
    occurrence: Option<LootContainerOccurrence>,
    admission_origin: Option<FieldOrigin>,
    ancestor_selection_origin: Option<FieldOrigin>,
    placement_survey: Option<FieldOrigin>,
    failed_source_uses: Vec<SourceUseRef>,
    entity_link_survey: Option<FieldOrigin>,
    records: CatalogRecordCoverage,
    requested_aliases: Vec<u32>,
}

impl Outcome {
    fn has_complete_link_survey(&self) -> bool {
        self.entity_link_survey
            .as_ref()
            .is_some_and(FieldOrigin::source_set_complete)
    }
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            status: LootContainerStatus::Unavailable,
            row: None,
            occurrence: None,
            admission_origin: None,
            ancestor_selection_origin: None,
            placement_survey: None,
            failed_source_uses: Vec::new(),
            entity_link_survey: None,
            records: CatalogRecordCoverage::default(),
            requested_aliases: Vec::new(),
        }
    }
}

pub(crate) fn read_physical_container<'source>(
    archive: &Archive,
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    query: LootContainerQuery<'_>,
    cancel: Option<&CancelToken>,
) -> Result<LootContainerSnapshot, CatalogError> {
    let capture = EntryCapture::for_input(input);
    let index = Index::new(archive);
    let mut systems = SystemOrigins::try_new(archive, &capture, &index)?;
    let anchor = systems.at(position(query.system_entry_index)?)?.clone();
    let outcome = if let Some(folder) = anchor.value() {
        select_row(
            archive,
            forge,
            input,
            query,
            folder,
            (&index, &capture),
            cancel,
        )?
    } else if anchor.origin().source_set_complete()
        && matches!(
            anchor.state(),
            SourceValueState::Absent | SourceValueState::Invalid
        )
    {
        Outcome {
            status: LootContainerStatus::NotFound,
            ..Outcome::default()
        }
    } else {
        Outcome::default()
    };
    checkpoint(cancel)?;
    let source_inventory = capture.source_uses().finish_with(|| checkpoint(cancel))?;
    Ok(LootContainerSnapshot {
        build: archive.build().cloned(),
        system_anchor: anchor,
        status: outcome.status,
        row: outcome.row,
        occurrence: outcome.occurrence,
        admission_origin: outcome.admission_origin,
        ancestor_selection_origin: outcome.ancestor_selection_origin,
        placement_survey: outcome.placement_survey,
        failed_source_uses: outcome.failed_source_uses,
        entity_link_survey: outcome.entity_link_survey,
        records: outcome.records,
        requested_aliases: outcome.requested_aliases,
        source_inventory,
    })
}

fn select_row<'source>(
    archive: &Archive,
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    query: LootContainerQuery<'_>,
    folder: &str,
    (index, capture): (&Index<'_>, &EntryCapture<'_>),
    cancel: Option<&CancelToken>,
) -> Result<Outcome, CatalogError> {
    let mut outcome = Outcome::default();
    let Some(requested) = outcome.admit_ancestor(forge, input.origins(), query, cancel)? else {
        return Ok(outcome);
    };
    let (mut placements, walked) =
        linked_inputs(archive, index, capture, forge, query, folder, cancel)?;
    if let Some(survey) = placements.loot_survey.take() {
        outcome.placement_survey = Some(survey.origin);
        outcome.failed_source_uses.extend(survey.failed_source_uses);
    }
    outcome.entity_link_survey = walked.document_census;
    let roots = placements
        .opened
        .iter()
        .filter(|seed| seed.system.as_ref() == folder);
    let candidates = lookup::collect_candidates(roots, &walked.summaries, query);
    let demands = candidates
        .iter()
        .filter_map(|candidate| candidate.root.under)
        .chain(requested.guid());
    let facts = AncestorFacts::read(
        forge,
        input,
        demands,
        &placements,
        query.catalog_filter,
        cancel,
    )?;
    if outcome.admit_requested(requested, &facts) {
        let desired = ContainerKey::new(
            query.multi_config_guid,
            query.container,
            folder,
            requested.guid().map(CatalogGuid),
        );
        let objects = facts.objects();
        complete_match(
            archive,
            (index, capture),
            &candidates,
            &objects,
            &desired,
            cancel,
            &mut outcome,
        )?;
    }

    outcome.failed_source_uses.extend(facts.failed_source_uses);
    outcome.records = facts.coverage;
    Ok(outcome)
}

fn linked_inputs(
    archive: &Archive,
    index: &Index<'_>,
    capture: &EntryCapture<'_>,
    forge: &DataForge<'_>,
    query: LootContainerQuery<'_>,
    folder: &str,
    cancel: Option<&CancelToken>,
) -> Result<(Placements, loot_walk::Walked), CatalogError> {
    let targets = placement_targets(forge, query, cancel)?;
    let placements =
        Placements::read_for_objects(archive, cancel, query.placement_budget, capture, &targets)?;
    let walked = loot_walk::visit_documents(
        archive,
        index,
        placements
            .opened
            .iter()
            .filter(|seed| seed.system.as_ref() == folder)
            .map(|seed| seed.entry),
        cancel,
        capture,
        None::<DocumentVisit<DocumentCallback>>,
    )?;
    Ok((placements, walked))
}

impl Outcome {
    fn admit_requested(&mut self, requested: RequestedOwner, facts: &AncestorFacts) -> bool {
        let RequestedOwner::Record { index, guid } = requested else {
            return true;
        };
        self.requested_aliases = facts
            .admitted_aliases(guid)
            .iter()
            .map(|owner| owner.index)
            .collect();
        if !self.requested_aliases.contains(&index) {
            return false;
        }
        match facts.owner(Some(guid)) {
            AncestorOwner::Unique(owner) => owner == index,
            AncestorOwner::Ambiguous(_) => {
                self.status = LootContainerStatus::Ambiguous;
                false
            }
            _ => false,
        }
    }
}

fn placement_targets(
    forge: &DataForge<'_>,
    query: LootContainerQuery<'_>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<Guid>, CatalogError> {
    let mut targets = Vec::new();
    let mut coverage = CatalogRecordCoverage::default();
    for record in forge.records() {
        checkpoint(cancel)?;
        if starmap_name_owners::selected(&record, query.catalog_filter, &mut coverage) {
            targets.push(record.guid());
        }
    }
    targets.sort_unstable();
    targets.dedup();
    Ok(targets)
}

fn complete_match(
    archive: &Archive,
    (index, capture): (&Index<'_>, &EntryCapture<'_>),
    candidates: &[lookup::Candidate<'_>],
    objects: &Objects<'_>,
    desired: &ContainerKey<'_>,
    cancel: Option<&CancelToken>,
    outcome: &mut Outcome,
) -> Result<(), CatalogError> {
    let survey_complete = outcome
        .placement_survey
        .as_ref()
        .is_some_and(FieldOrigin::source_set_complete);
    let (matched, ancestors_complete) = lookup::first_match(
        candidates,
        objects,
        desired,
        capture.source_uses(),
        survey_complete,
        cancel,
    )?;
    complete_row(
        archive,
        (index, capture),
        matched.as_ref(),
        objects,
        cancel,
        survey_complete && ancestors_complete,
        outcome,
    )
}

fn complete_row(
    archive: &Archive,
    (index, capture): (&Index<'_>, &EntryCapture<'_>),
    matched: Option<&SelectedPlacement<'_>>,
    objects: &Objects<'_>,
    cancel: Option<&CancelToken>,
    complete: bool,
    outcome: &mut Outcome,
) -> Result<(), CatalogError> {
    let Some(SelectedPlacement {
        root,
        placed,
        location,
    }) = matched
    else {
        if complete && outcome.has_complete_link_survey() {
            outcome.status = LootContainerStatus::NotFound;
        }
        return Ok(());
    };
    let (tags, _) = read_tags(archive, placed.target.entry().into_iter(), cancel, capture)?;
    let mut systems = SystemOrigins::try_new(archive, capture, index)?;
    let mut parents = ParentOrigins::new(objects, capture.source_uses());
    let system = systems.project(root)?;
    let object = parents.project(root, *location, &system)?;
    let row = row(
        placed,
        PlacementValues { system, object },
        placed.target.entry().and_then(|entry| tags.get(&entry)),
        capture.source_uses(),
    )?;
    outcome.status = if complete
        && outcome.has_complete_link_survey()
        && row.multi_config_guid.origin().source_set_complete()
        && row.container.origin().source_set_complete()
        && row.system.origin().source_set_complete()
    {
        LootContainerStatus::Found
    } else {
        LootContainerStatus::Unavailable
    };
    outcome.occurrence = Some(LootContainerOccurrence {
        system_entry_index: wire_index(root.system_entry)?,
        seed_entry_index: wire_index(root.entry)?,
        containing_entry_index: wire_index(placed.author.containing_entry_index)?,
        document_entry_index: wire_index(placed.author.document_entry_index)?,
    });
    outcome.row = Some(row);
    Ok(())
}

fn position(index: u32) -> Result<usize, ExportAccountingError> {
    usize::try_from(index).map_err(|_| ExportAccountingError::CounterOverflow)
}

fn wire_index(index: usize) -> Result<u32, ExportAccountingError> {
    u32::try_from(index).map_err(|_| ExportAccountingError::CounterOverflow)
}
