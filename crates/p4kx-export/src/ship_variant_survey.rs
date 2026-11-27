// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use p4kx_assembly::ArchiveFiles;
use p4kx_extraction::{
    Archive, CancelToken, DataForgeError, FieldOrigin, Record, SourcedValue, TreeBuilder,
};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::authored_fields::ship_source_read::DefinitionMembership;
use crate::digest::Digest;
use crate::guid_text::CatalogGuid;
use crate::variant_decision::VariantDecision;
use crate::variant_evidence::VariantEvidence;
use crate::variant_target;
use crate::variant_use_sets::VariantSourceUseSet;
use crate::variants::{ShipDefinitionKey, VariantBases};
use crate::{CatalogError, ExportAccountingError};

#[path = "ship_variant_survey_candidate.rs"]
mod candidate;
#[path = "ship_variant_catalog.rs"]
mod output;
use self::candidate::{Candidate, StructuralCandidate};
pub(crate) use self::output::project_for_catalog;

struct Selection {
    rows: HashSet<u32>,
    keys: HashSet<ShipDefinitionKey>,
}

struct Survey<'source> {
    selection: Selection,
    selected: HashMap<u32, Owner>,
    cohorts: HashMap<ShipDefinitionKey, Definition<'source>>,
    unknown: Vec<VariantEvidence>,
}

enum Owner {
    Local(VariantEvidence),
    Compared {
        definition: ShipDefinitionKey,
        digest: Digest,
        guid: CatalogGuid,
    },
}

struct Definition<'source> {
    first: u32,
    structural: Vec<VariantEvidence>,
    ranks: HashMap<Digest, Vec<VariantEvidence>>,
    bases: VariantBases<'source, Digest>,
}

pub(crate) struct Results {
    pub(crate) decisions: HashMap<u32, VariantDecision>,
}

/// Chooses the exact physical bases for requested Ship owners from the unfiltered Build.
pub(crate) fn read<'row, 'source: 'row>(
    archive: &'source Archive,
    input: &'source OriginalCatalogInput<'source, 'source>,
    rows: impl IntoIterator<Item = (u32, &'row SourcedValue<&'source str>)>,
    cancel: Option<&CancelToken>,
) -> Result<Results, CatalogError> {
    let mut survey = Survey {
        selection: Selection {
            rows: HashSet::new(),
            keys: HashSet::new(),
        },
        selected: HashMap::new(),
        cohorts: HashMap::new(),
        unknown: Vec::new(),
    };
    for (index, definition) in rows {
        match DefinitionMembership::of(definition) {
            DefinitionMembership::Comparable(key) => {
                survey.selection.keys.insert(key);
                survey.selection.rows.insert(index);
            }
            membership => {
                let sources = &input.origins().uses;
                let proof = sources.origin_with_origins(
                    [definition.origin()],
                    matches!(membership, DefinitionMembership::Excluded),
                )?;
                survey
                    .selected
                    .insert(index, Owner::Local(VariantEvidence::from_origin(proof)));
            }
        }
    }
    if survey.selection.rows.is_empty() {
        return survey.finish(input, None);
    }
    let files = ArchiveFiles::new(archive).with_verified_entries();
    let mut trees = TreeBuilder::new();
    let mut census = 0;
    for record in input.forge().records() {
        checkpoint(cancel)?;
        survey.observe(input, &files, &record, &mut trees, cancel)?;
        census += 1;
    }
    survey.finish(input, Some(candidate::definition_census(input, census)?))
}

impl<'source> Survey<'source> {
    fn observe(
        &mut self,
        input: &'source OriginalCatalogInput<'source, 'source>,
        files: &ArchiveFiles<'source>,
        record: &Record<'source>,
        trees: &mut TreeBuilder<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let requested = self.selection.rows.contains(&record.index());
        match candidate::admission(input, record)? {
            Candidate::Other => {
                if requested {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
            }
            Candidate::Unknown(evidence) => self.record_unknown(record, requested, evidence),
            Candidate::Admitted(header) => {
                let tree = match trees.build(record) {
                    Ok(tree) => tree,
                    Err(DataForgeError::ObservationStopped) => return Err(CatalogError::Cancelled),
                    Err(_) => {
                        let evidence = candidate::unreadable_body(input, record, &header)?;
                        self.record_unknown(record, requested, evidence);
                        return Ok(());
                    }
                };
                let candidate =
                    candidate::read(input, files, record, &tree, &self.selection.keys, cancel)?;
                self.retain(input, record, requested, &header, candidate)?;
            }
        }
        Ok(())
    }

    fn record_unknown(&mut self, record: &Record<'_>, requested: bool, evidence: VariantEvidence) {
        if requested {
            self.selected
                .insert(record.index(), Owner::Local(evidence.clone()));
        }
        self.unknown.push(evidence);
    }

    fn retain(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
        requested: bool,
        header: &VariantEvidence,
        candidate: candidate::Membership<'source>,
    ) -> Result<(), CatalogError> {
        let sources = &input.origins().uses;
        match candidate {
            candidate::Membership::Unknown(definition) => {
                let evidence = VariantEvidence::join(sources, [header, &definition], false)?;
                self.record_unknown(record, requested, evidence);
            }
            candidate::Membership::Unrelated => {
                if requested {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
            }
            candidate::Membership::Reached {
                definition,
                structural,
            } => {
                let cohort = self
                    .cohorts
                    .entry(definition.clone())
                    .or_insert_with(|| Definition::new(record.index()));
                let evidence =
                    VariantEvidence::join(sources, [header, &structural.evidence], true)?;
                let guid = CatalogGuid(record.guid());
                let digest = structural.digest;
                cohort.observe(record.index(), structural, evidence.clone());
                if requested {
                    let owner = digest.map_or(Owner::Local(evidence), |digest| Owner::Compared {
                        definition,
                        digest,
                        guid,
                    });
                    self.selected.insert(record.index(), owner);
                }
            }
        }
        Ok(())
    }

    fn finish(
        self,
        input: &OriginalCatalogInput<'_, '_>,
        census: Option<FieldOrigin>,
    ) -> Result<Results, CatalogError> {
        if self
            .selection
            .rows
            .iter()
            .any(|index| !self.selected.contains_key(index))
        {
            return Err(ExportAccountingError::MissingFieldInspection.into());
        }
        let sources = &input.origins().uses;
        let census = census.map(VariantEvidence::from_origin);
        let all = VariantEvidence::join(sources, self.unknown.iter().chain(census.iter()), true)?;
        let mut bases = HashMap::new();
        let mut decisions = HashMap::new();
        for (index, owner) in self.selected {
            let decision = match owner {
                Owner::Local(proof) => bind(input, &proof, None, None)?,
                Owner::Compared {
                    definition,
                    digest,
                    guid,
                } => {
                    let cohort = self
                        .cohorts
                        .get(&definition)
                        .ok_or(ExportAccountingError::MissingFieldInspection)?;
                    let base = match bases.entry(definition) {
                        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            entry.insert(cohort.shared(input, &all)?)
                        }
                    };
                    let ranks = cohort
                        .ranks
                        .get(&digest)
                        .ok_or(ExportAccountingError::MissingFieldInspection)?;
                    let proof =
                        VariantEvidence::join(sources, ranks.iter().chain([&base.1]), true)?;
                    bind(
                        input,
                        &proof,
                        cohort.bases.base_for(&digest, guid),
                        Some(&base.0),
                    )?
                }
            };
            decisions.insert(index, decision);
        }
        Ok(Results { decisions })
    }
}

impl<'source> Definition<'source> {
    fn new(first: u32) -> Self {
        Self {
            first,
            structural: Vec::new(),
            ranks: HashMap::new(),
            bases: VariantBases::new(),
        }
    }

    fn observe(
        &mut self,
        index: u32,
        candidate: StructuralCandidate<'source>,
        evidence: VariantEvidence,
    ) {
        self.first = self.first.min(index);
        self.structural.push(evidence);
        if let Some(digest) = candidate.digest
            && let Some(rank) = candidate.rank
        {
            if rank.evidence.origin.source_set_complete() {
                self.bases.observe(digest, rank.rank);
            }
            self.ranks.entry(digest).or_default().push(rank.evidence);
        }
    }

    fn shared(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        all: &VariantEvidence,
    ) -> Result<(Arc<VariantSourceUseSet>, VariantEvidence), CatalogError> {
        let proof = VariantEvidence::join(
            &input.origins().uses,
            self.structural.iter().chain([all]),
            true,
        )?;
        let set = Arc::new(VariantSourceUseSet::new(self.first, proof.origin.clone()));
        Ok((set, proof))
    }
}

fn bind(
    input: &OriginalCatalogInput<'_, '_>,
    proof: &VariantEvidence,
    choice: Option<crate::variants::VariantMember>,
    basis: Option<&Arc<VariantSourceUseSet>>,
) -> Result<VariantDecision, CatalogError> {
    let expanded = variant_target::bind(input, choice, &proof.origin)?;
    let decision = if let Some(basis) = basis {
        VariantDecision::shared(expanded, basis)
    } else {
        VariantDecision::local(expanded)
    };
    Ok(decision.with_failed_source_uses(proof.failed_source_uses.clone()))
}
