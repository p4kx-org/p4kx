// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "commodity_query_backfill.rs"]
mod backfill;
#[path = "commodity_query_entities.rs"]
mod entities;
#[path = "commodity_query_plan.rs"]
mod plan;

use p4kx_extraction::{Archive, CancelToken, DataForge, Language};

use super::snapshot::CommoditySnapshot;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::mining_definitions::dependencies::Dependencies;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session;
use crate::catalog_query_session::CatalogQuerySource;
use crate::commodity;
use crate::guid_text;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

pub(super) struct Query<'query> {
    pub(super) archive: &'query Archive,
    pub(super) entry: CatalogQuerySource<'query>,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) selection: CatalogRecordSelection,
    pub(super) requested_languages: &'query [Language],
    pub(super) cancel: Option<&'query CancelToken>,
}

impl Query<'_> {
    pub(super) fn run<T>(
        self,
        inspect: impl FnOnce(CommoditySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        let mut plan = plan::Plan::read(self.forge, self.selection, self.cancel)?;
        let languages = if plan.entities.is_empty() && plan.resources.is_empty() {
            &[]
        } else {
            self.requested_languages
        };
        let sources = TextSources::read_languages(self.archive, languages)?;
        catalog_query_session::checkpoint(self.cancel)?;
        let origins = self.entry.with_texts(self.archive, &sources)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let texts = Texts::parse(&sources)?;
        let registry = CatalogTextRegistry::default();
        let projection =
            CatalogProjection::new(&texts, &input, &registry).for_kind(Kind::Commodities);
        let mut groups = Dependencies::new(None);
        let mut selection = entities::Selection::read(
            &mut plan,
            self.forge,
            &projection,
            &mut groups,
            self.cancel,
        )?;
        let dependency_coverage = backfill::append(
            plan.resources,
            &mut plan.coverage,
            &mut selection,
            &projection,
            &mut groups,
            self.cancel,
        )?;
        guid_text::sort_by_guid(&mut selection.rows, |row| row.guid);
        let tables = commodity::tables(self.forge, selection.rows);
        let result = projection.finish_rows()?;
        let source_inventory = origins
            .uses
            .finish_with(|| catalog_query_session::checkpoint(self.cancel))?;
        catalog_query_session::checkpoint(self.cancel)?;
        Ok(inspect(CommoditySnapshot {
            build: self.archive.build().cloned(),
            availability: tables.definitions.availability,
            definitions: tables.definitions.rows,
            presentations: tables.presentation.rows,
            public_names: result.public_names,
            game_texts: result.game_texts,
            requested_languages: self.requested_languages.to_vec(),
            available_languages: languages
                .iter()
                .copied()
                .filter(|language| sources.entry(*language).is_some())
                .collect(),
            localization_notes: texts.notes().to_vec(),
            source_inventory,
            coverage: plan.coverage,
            dependency_coverage,
        }))
    }
}

fn append_coverage(target: &mut CatalogRecordCoverage, mut from: CatalogRecordCoverage) {
    target
        .unreadable_headers
        .append(&mut from.unreadable_headers);
    target.unreadable_bodies.append(&mut from.unreadable_bodies);
    target.incomplete_fields.append(&mut from.incomplete_fields);
    target.skipped_leftovers += from.skipped_leftovers;
    for indices in [
        &mut target.unreadable_headers,
        &mut target.unreadable_bodies,
        &mut target.incomplete_fields,
    ] {
        indices.sort_unstable();
        indices.dedup();
    }
}
