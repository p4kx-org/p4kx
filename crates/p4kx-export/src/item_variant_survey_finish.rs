// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;

use p4kx_extraction::{SourceUseRef, SourceUses};

use super::inputs::component_census;
use super::{Cohort, VariantRead};
use crate::build::original_input::OriginalCatalogInput;
use crate::item_variant::ItemKey;
use crate::variant_decision::VariantDecision;
use crate::variant_evidence::VariantEvidence;
use crate::variant_target;
use crate::variant_use_sets::VariantSourceUseSet;
use crate::variants::VariantMember;
use crate::{CatalogError, ExportAccountingError};

pub(super) struct GroupComparison {
    first: u32,
    evidence: Vec<VariantEvidence>,
}

impl GroupComparison {
    pub(super) const fn new(index: u32) -> Self {
        Self {
            first: index,
            evidence: Vec::new(),
        }
    }

    pub(super) fn observe(&mut self, index: u32, evidence: VariantEvidence) {
        self.first = self.first.min(index);
        self.evidence.push(evidence);
    }
}

struct SharedDecision {
    set: Arc<VariantSourceUseSet>,
    no_target: VariantDecision,
    winner: Option<(VariantMember, VariantDecision)>,
    failed_source_uses: Vec<SourceUseRef>,
}

impl SharedDecision {
    fn new(
        input: &OriginalCatalogInput<'_, '_>,
        index: u32,
        evidence: VariantEvidence,
    ) -> Result<Self, CatalogError> {
        let set = Arc::new(VariantSourceUseSet::new(index, evidence.origin));
        let expanded = variant_target::bind(input, None, set.origin())?;
        Ok(Self {
            no_target: VariantDecision::shared(expanded, &set),
            set,
            winner: None,
            failed_source_uses: evidence.failed_source_uses,
        })
    }

    fn bind(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        choice: Option<VariantMember>,
    ) -> Result<VariantDecision, CatalogError> {
        let Some(choice) = choice.filter(|_| self.set.origin().source_set_complete()) else {
            return Ok(self.no_target.clone());
        };
        if let Some((winner, cell)) = &self.winner {
            if *winner != choice {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            return Ok(cell.clone());
        }
        let expanded = variant_target::bind(input, Some(choice), self.set.origin())?;
        let cell = VariantDecision::shared(expanded, &self.set);
        self.winner = Some((choice, cell.clone()));
        Ok(cell)
    }
}

impl<'source> Cohort<'source> {
    pub(super) fn finish(
        mut self,
        input: &OriginalCatalogInput<'source, 'source>,
        inspected_headers: Option<usize>,
    ) -> Result<HashMap<u32, VariantRead>, CatalogError> {
        let uses = &input.origins().uses;
        let header_scope = inspected_headers
            .map(|count| component_census(input, count).map(VariantEvidence::from_origin))
            .transpose()?;
        let census = VariantEvidence::join(
            uses,
            self.census_evidence.iter().chain(header_scope.iter()),
            true,
        )?;
        let mut group_proofs: HashMap<_, SharedDecision> = HashMap::new();
        std::mem::take(&mut self.owners)
            .into_iter()
            .map(|(index, owner)| {
                let variant = if let Some(key) = owner.key {
                    let proof = match group_proofs.entry(key) {
                        Entry::Occupied(entry) => entry.into_mut(),
                        Entry::Vacant(entry) => {
                            let (comparison, evidence) = self.group_evidence(key, uses, &census)?;
                            if comparison.evidence.len() == 1 {
                                return Ok((index, local_variant(input, evidence)?));
                            }
                            entry.insert(SharedDecision::new(input, comparison.first, evidence)?)
                        }
                    };
                    VariantRead {
                        decision: proof.bind(input, self.bases.base_for(&key, owner.guid))?,
                        failed_source_uses: proof.failed_source_uses.clone(),
                    }
                } else {
                    local_variant(input, owner.evidence)?
                };
                Ok((index, variant))
            })
            .collect()
    }

    fn group_evidence(
        &self,
        key: ItemKey<'source>,
        uses: &SourceUses,
        census: &VariantEvidence,
    ) -> Result<(&GroupComparison, VariantEvidence), CatalogError> {
        let comparison = self
            .group_origins
            .get(&key)
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        let failed = self
            .failed_stats
            .get(&key.components())
            .into_iter()
            .flatten();
        let uncertain = self
            .uncertain_types
            .get(key.components().kind())
            .into_iter()
            .flatten();
        let evidence = VariantEvidence::join(
            uses,
            comparison
                .evidence
                .iter()
                .chain(failed)
                .chain(uncertain)
                .chain([census]),
            true,
        )?;
        Ok((comparison, evidence))
    }
}

fn local_variant(
    input: &OriginalCatalogInput<'_, '_>,
    evidence: VariantEvidence,
) -> Result<VariantRead, CatalogError> {
    let expanded = variant_target::bind(input, None, &evidence.origin)?;
    Ok(VariantRead {
        decision: VariantDecision::local(expanded),
        failed_source_uses: evidence.failed_source_uses,
    })
}
