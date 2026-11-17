// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Record, RecordTree, TreeBuilder};

use super::super::{CatalogRecordCoverage, CatalogRecordSelection, unlocalized_record_query};
use super::{ReputationRows, ReputationSelection};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_query_session::checkpoint;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::reputation_faction_definition::{
    self as factions, FactionReputationDefinition,
};
use crate::catalog_v3::reputation_scope_definition::{self as scopes, ScopeDefinition};
use crate::catalog_v3::reputation_standing_definition::{self as standings, StandingDefinition};
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::{CatalogError, Kind};

struct SelectedRecords<'source> {
    kind: Kind,
    records: Vec<Record<'source>>,
    coverage: CatalogRecordCoverage,
}

impl<'source> SelectedRecords<'source> {
    fn read(
        forge: &'source DataForge<'_>,
        selection: Option<CatalogRecordSelection>,
        kind: Kind,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Self>, CatalogError> {
        let Some(selection) = selection else {
            return Ok(None);
        };
        let mut coverage = CatalogRecordCoverage::default();
        let mut records = Vec::new();
        selection.for_each(forge, cancel, |record| {
            if unlocalized_record_query::selected(record, kind, &mut coverage) {
                records.push(*record);
            }
            Ok(())
        })?;
        records.sort_by_cached_key(|record| (CatalogGuid(record.guid()).text(), record.index()));
        Ok(Some(Self {
            kind,
            records,
            coverage,
        }))
    }

    fn project<Row>(
        mut self,
        trees: &mut TreeBuilder<'source>,
        projection: &CatalogProjection<'_, 'source>,
        cancel: Option<&CancelToken>,
        mut read: impl FnMut(
            &RecordTree<'source>,
            &CatalogProjection<'_, 'source>,
            &RecordObservation<'_, 'source>,
        ) -> Result<(Row, Vec<FieldError>), CatalogError>,
    ) -> Result<(Vec<Row>, CatalogRecordCoverage), CatalogError> {
        let projection = projection.for_kind(self.kind);
        let mut rows = Vec::new();
        for record in self.records {
            checkpoint(cancel)?;
            let Ok(tree) = trees.build(&record) else {
                self.coverage.unreadable_bodies.push(record.index());
                continue;
            };
            let admission = RecordObservation::capture(projection.original(), &record)?;
            let (row, notes) = read(&tree, &projection, &admission)?;
            if notes
                .iter()
                .any(|note| note.description().issue.blocks_projection())
            {
                self.coverage.incomplete_fields.push(record.index());
            }
            rows.push(row);
        }
        Ok((rows, self.coverage))
    }
}

pub(super) struct SelectedReputation<'source> {
    faction_reputations: Option<SelectedRecords<'source>>,
    scopes: Option<SelectedRecords<'source>>,
    standings: Option<SelectedRecords<'source>>,
}

impl<'source> SelectedReputation<'source> {
    pub(super) fn read(
        forge: &'source DataForge<'_>,
        selection: ReputationSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            faction_reputations: SelectedRecords::read(
                forge,
                selection.faction_reputations,
                Kind::FactionReputations,
                cancel,
            )?,
            scopes: SelectedRecords::read(forge, selection.scopes, Kind::ReputationScopes, cancel)?,
            standings: SelectedRecords::read(
                forge,
                selection.standings,
                Kind::ReputationStandings,
                cancel,
            )?,
        })
    }

    pub(super) fn has_records(&self) -> bool {
        [&self.faction_reputations, &self.scopes, &self.standings]
            .into_iter()
            .flatten()
            .any(|selection| !selection.records.is_empty())
    }

    pub(super) fn project(
        self,
        forge: &DataForge<'_>,
        projection: &CatalogProjection<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<ProjectedReputation<'source>, CatalogError> {
        let mut trees = TreeBuilder::new();
        let faction_reputations = self
            .faction_reputations
            .map(|selected| {
                let (rows, coverage) = selected.project(
                    &mut trees,
                    projection,
                    cancel,
                    |tree, projection, admission| {
                        let result = factions::read(tree, projection, admission, cancel)?;
                        Ok((result.value, result.notes))
                    },
                )?;
                Ok::<_, CatalogError>(reputation_rows(
                    factions::tables(forge, rows).definitions,
                    coverage,
                ))
            })
            .transpose()?;
        let scopes = self
            .scopes
            .map(|selected| {
                let (rows, coverage) = selected.project(
                    &mut trees,
                    projection,
                    cancel,
                    |tree, projection, admission| {
                        let result = scopes::read(tree, projection, admission, cancel)?;
                        Ok((result.value, result.notes))
                    },
                )?;
                Ok::<_, CatalogError>(reputation_rows(scopes::table(forge, rows), coverage))
            })
            .transpose()?;
        let standings = self
            .standings
            .map(|selected| {
                let (rows, coverage) = selected.project(
                    &mut trees,
                    projection,
                    cancel,
                    |tree, projection, admission| {
                        let result = standings::read(tree, projection, admission, cancel)?;
                        Ok((result.value, result.notes))
                    },
                )?;
                Ok::<_, CatalogError>(reputation_rows(standings::table(forge, rows), coverage))
            })
            .transpose()?;
        Ok(ProjectedReputation {
            faction_reputations,
            scopes,
            standings,
        })
    }
}

pub(super) struct ProjectedReputation<'source> {
    pub(super) faction_reputations: Option<ReputationRows<FactionReputationDefinition<'source>>>,
    pub(super) scopes: Option<ReputationRows<ScopeDefinition<'source>>>,
    pub(super) standings: Option<ReputationRows<StandingDefinition<'source>>>,
}

fn reputation_rows<Row>(
    table: CatalogTable<Row>,
    coverage: CatalogRecordCoverage,
) -> ReputationRows<Row> {
    ReputationRows {
        availability: table.availability,
        rows: table.rows,
        coverage,
    }
}
