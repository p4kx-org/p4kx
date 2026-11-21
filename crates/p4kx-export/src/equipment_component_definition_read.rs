// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, CancelToken, DataForge, DataForgeEntry, DataForgeError, Language, TreeBuilder,
};

use super::{ComponentDefinitionSnapshot, EquipmentDependencyCoverage, EquipmentSnapshot};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::component_definition_sources::Definition;
use crate::record_observation::RecordObservation;
use crate::texts::{TextSources, Texts};

pub(super) struct Query<'query> {
    pub(super) archive: &'query Archive,
    pub(super) entry: &'query DataForgeEntry,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) selection: CatalogRecordSelection,
    pub(super) languages: &'query [Language],
    pub(super) cancel: Option<&'query CancelToken>,
}

impl Query<'_> {
    pub(super) fn read<F: Definition, T>(
        self,
        inspect: impl for<'source> FnOnce(ComponentDefinitionSnapshot<'source, F::Row<'source>>) -> T,
    ) -> Result<T, CatalogError> {
        let requested_languages = if F::TEXT_OWNER.is_some() {
            self.languages
        } else {
            &[]
        };
        let has_owner = !requested_languages.is_empty() && self.has_owner(F::SOURCE_TYPES)?;
        let selected_languages = if has_owner { requested_languages } else { &[] };
        let sources = TextSources::read_languages(self.archive, selected_languages)?;
        let origins = CatalogOrigins::new(self.archive, self.entry, &sources)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let texts = Texts::parse(&sources)?;
        let registry = CatalogTextRegistry::default();
        let projection = CatalogProjection::new(&texts, &input, &registry);
        let projection = match F::TEXT_OWNER {
            Some(owner) => projection.for_text_owner(owner),
            None => projection,
        };
        let (source_types, ledger) =
            F::observe_types(self.forge, &projection, self.selection, self.cancel)?;
        let mut dependencies = EquipmentDependencyCoverage::default();
        dependencies.finish(self.archive, &ledger)?;
        let (rows, coverage) = self.rows::<F>(&projection, &source_types)?;
        let table = CatalogTable::new(self.forge, &source_types, rows);
        let canonical = projection.finish_rows()?;
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(inspect(EquipmentSnapshot {
            build: self.archive.build().cloned(),
            availability: table.availability,
            rows: table.rows,
            public_names: canonical.public_names,
            game_texts: canonical.game_texts,
            requested_languages: requested_languages.to_vec(),
            available_languages: selected_languages
                .iter()
                .copied()
                .filter(|language| sources.entry(*language).is_some())
                .collect(),
            localization_notes: texts.notes().to_vec(),
            source_inventory,
            coverage,
            dependency_coverage: dependencies,
        }))
    }

    fn has_owner(&self, wanted_types: &[&str]) -> Result<bool, CatalogError> {
        let mut found = false;
        self.selection.for_each(self.forge, self.cancel, |record| {
            found |= record
                .type_name()
                .is_ok_and(|name| wanted_types.contains(&name));
            Ok(())
        })?;
        Ok(found)
    }

    fn rows<'source, F: Definition>(
        &'source self,
        projection: &CatalogProjection<'_, 'source>,
        source_types: &[&str],
    ) -> Result<(Vec<F::Row<'source>>, CatalogRecordCoverage), CatalogError> {
        let mut rows = Vec::new();
        let mut coverage = CatalogRecordCoverage::default();
        let mut trees = TreeBuilder::new();
        self.selection.for_each(self.forge, self.cancel, |record| {
            checkpoint(self.cancel)?;
            let Ok(source_type) = record.type_name() else {
                coverage.unreadable_headers.push(record.index());
                return Ok(());
            };
            if !source_types.contains(&source_type) {
                return Ok(());
            }
            let admission = RecordObservation::capture(projection.original(), record)?;
            match trees.build(record) {
                Ok(tree) => {
                    let read = F::project(&tree, projection, &admission, self.cancel)?;
                    if !read.selection_complete {
                        coverage.incomplete_fields.push(record.index());
                    }
                    rows.push(read.value);
                }
                Err(DataForgeError::ObservationStopped) => return Err(CatalogError::Cancelled),
                Err(_) => coverage.unreadable_bodies.push(record.index()),
            }
            Ok(())
        })?;
        Ok((rows, coverage))
    }
}
