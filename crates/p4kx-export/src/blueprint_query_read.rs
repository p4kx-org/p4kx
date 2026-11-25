// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Archive, CancelToken, CanonicalReadObserver, DataForge, Language};

use super::plan::Plan;
use super::project;
use super::snapshot::BlueprintSnapshot;
use crate::blueprint_catalog_rows;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_query_session::{CatalogQuerySource, checkpoint};
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

pub(super) struct Query<'query> {
    pub(super) archive: &'query Archive,
    pub(super) entry: CatalogQuerySource<'query>,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) selection: CatalogRecordSelection,
    pub(super) languages: &'query [Language],
    pub(super) cancel: Option<&'query CancelToken>,
    pub(super) observer: Option<Arc<dyn CanonicalReadObserver>>,
}

impl Query<'_> {
    pub(super) fn run<T>(
        self,
        inspect: impl FnOnce(BlueprintSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        let plan = Plan::read(
            self.forge,
            self.selection,
            self.cancel,
            self.observer.clone(),
        )?;
        let languages = if plan.owners.is_empty() {
            &[]
        } else {
            self.languages
        };
        let sources = TextSources::read_languages(self.archive, languages)?;
        checkpoint(self.cancel)?;
        let origins = self.entry.with_texts(self.archive, &sources)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let texts = Texts::parse(&sources)?;
        let registry = CatalogTextRegistry::default();
        let projection =
            CatalogProjection::new(&texts, &input, &registry).for_kind(Kind::Blueprints);
        let rows = project::project(plan, &projection, self.observer, self.cancel)?;
        let canonical = projection.finish_rows()?;
        let inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(inspect(BlueprintSnapshot {
            build: self.archive.build().cloned(),
            availability: blueprint_catalog_rows::availability(self.forge, rows.definitions.len()),
            definitions: rows.definitions,
            tiers: rows.tiers,
            presentations: rows.presentations,
            public_names: canonical.public_names,
            game_texts: canonical.game_texts,
            requested_languages: self.languages.to_vec(),
            available_languages: languages
                .iter()
                .copied()
                .filter(|language| sources.entry(*language).is_some())
                .collect(),
            localization_notes: texts.notes().to_vec(),
            source_inventory: inventory,
            coverage: rows.coverage,
            dependency_coverage: rows.dependency_coverage,
            unsupported_ranges: rows.unsupported_ranges,
        }))
    }
}
