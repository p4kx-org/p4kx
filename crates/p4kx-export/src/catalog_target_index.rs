// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use p4kx_extraction::Guid;

use crate::catalog_row::{CatalogRow, CatalogRowExclusion, ExcludedCatalogRow};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_field_inspection::FieldInspection;
use crate::export_ledger::{ExportAccountingError, SourceDisposition, SourceObservation};
use crate::selection::Kind;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Identity<'source> {
    kind: usize,
    guid: Guid,
    path: &'source str,
}

impl<'source> Identity<'source> {
    fn row(row: &'source CatalogRow) -> Self {
        Self {
            kind: row.kind.index(),
            guid: row.guid,
            path: &row.record_path,
        }
    }
}

type Origins<'source> = HashMap<Identity<'source>, BTreeSet<u32>>;

#[derive(Clone, Copy)]
enum OriginScope {
    Emitted,
    Excluded,
}

/// Actual target-row materializations resolved onto unique physical source identities.
pub(crate) struct TargetIndex {
    pub(crate) emitted: HashSet<(Kind, u32)>,
    pub(crate) emitted_by_guid: HashMap<(Kind, Guid), BTreeSet<u32>>,
    pub(crate) ambiguous: HashSet<(Kind, Guid)>,
    pub(crate) ambiguous_sources: Vec<(Kind, u32)>,
    pub(crate) excluded: HashMap<(Kind, u32), CatalogRowExclusion>,
}

impl TargetIndex {
    pub(crate) fn new(
        rows: &[CatalogRow],
        excluded: &[ExcludedCatalogRow],
        sources: &[SourceObservation],
        canonical: &BTreeMap<u32, FieldInspection>,
    ) -> Result<Self, ExportAccountingError> {
        let selected = origins(sources, canonical, OriginScope::Emitted)?;
        let all = origins(sources, canonical, OriginScope::Excluded)?;
        let mut index = Self {
            emitted: HashSet::new(),
            emitted_by_guid: HashMap::new(),
            ambiguous: HashSet::new(),
            ambiguous_sources: Vec::new(),
            excluded: HashMap::new(),
        };
        index.emitted_rows(rows, &selected)?;
        index.excluded_rows(excluded, &all)?;
        for (kind, guid) in &index.ambiguous {
            if let Some(origins) = index.emitted_by_guid.get(&(*kind, *guid)) {
                index
                    .ambiguous_sources
                    .extend(origins.iter().map(|physical| (*kind, *physical)));
            }
        }
        index
            .ambiguous_sources
            .sort_by_key(|(kind, physical)| (kind.index(), *physical));
        index.ambiguous_sources.dedup();
        Ok(index)
    }

    fn emitted_rows(
        &mut self,
        rows: &[CatalogRow],
        selected: &Origins<'_>,
    ) -> Result<(), ExportAccountingError> {
        let mut identities = HashMap::<(Kind, Guid), usize>::new();
        for row in rows {
            let origins = selected
                .get(&Identity::row(row))
                .ok_or(ExportAccountingError::MissingCatalogRowSource { kind: row.kind })?;
            let count = identities.entry((row.kind, row.guid)).or_default();
            *count += 1;
            if origins.len() != 1 || *count != 1 {
                self.ambiguous.insert((row.kind, row.guid));
            }
            for physical in origins {
                self.emitted.insert((row.kind, *physical));
                self.emitted_by_guid
                    .entry((row.kind, row.guid))
                    .or_default()
                    .insert(*physical);
            }
        }
        Ok(())
    }

    fn excluded_rows(
        &mut self,
        excluded: &[ExcludedCatalogRow],
        all: &Origins<'_>,
    ) -> Result<(), ExportAccountingError> {
        for row in excluded {
            if row.reason == CatalogRowExclusion::Filter {
                continue;
            }
            let origins = all
                .get(&Identity::row(&row.row))
                .ok_or(ExportAccountingError::MissingCatalogRowSource { kind: row.row.kind })?;
            for physical in origins {
                self.excluded.insert((row.row.kind, *physical), row.reason);
            }
        }
        Ok(())
    }
}

fn origins<'source>(
    sources: &[SourceObservation],
    canonical: &'source BTreeMap<u32, FieldInspection>,
    scope: OriginScope,
) -> Result<Origins<'source>, ExportAccountingError> {
    let mut origins = Origins::new();
    for source in sources {
        let Some(kind) = source.kind else { continue };
        if !matches!(
            kind,
            Kind::Manufacturers
                | Kind::Factions
                | Kind::ContractTemplates
                | Kind::MissionBrokerEntries
                | Kind::GameplayProperties
                | Kind::Blueprints
                | Kind::LootPoolFilters
                | Kind::ExplosionDefinitions
                | Kind::AmmoDefinitions
                | Kind::Tags
                | Kind::Commodities
                | Kind::ResourceTypes
                | Kind::ResourceGroups
                | Kind::QualityDistributions
                | Kind::QualityQuantizations
                | Kind::StarmapObjects
                | Kind::ReputationStandings
        ) || !source.disposition.read_succeeded()
            || (matches!(scope, OriginScope::Emitted)
                && (source.role != ExportSourceRole::Selected
                    || source.disposition != SourceDisposition::Projected))
            || (matches!(scope, OriginScope::Excluded)
                && (source.role != ExportSourceRole::Selected
                    || source.disposition != SourceDisposition::SkippedAfterRead))
        {
            continue;
        }
        let proof = canonical
            .get(&source.record_index)
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        if proof.guid != source.guid
            || source
                .record_path
                .as_ref()
                .map(crate::HealthText::as_str)
                .is_some_and(|path| path != proof.record_path.as_str())
        {
            return Err(ExportAccountingError::ConflictingFieldInspection);
        }
        let identity = Identity {
            kind: kind.index(),
            guid: proof.guid,
            path: proof.record_path.as_str(),
        };
        origins
            .entry(identity)
            .or_default()
            .insert(source.record_index);
    }
    Ok(origins)
}

#[cfg(test)]
#[path = "catalog_broker_row_source_tests.rs"]
mod broker_source_tests;
