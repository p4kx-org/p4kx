// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{CancelToken, CanonicalReadObserver};

use super::request::Query;
use super::snapshot::ResourceGroupSnapshot;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_queries::unlocalized_record_query::plan::{Owner, Plan};
use crate::catalog_query_session::checkpoint;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::resource_group_catalog::{
    self, ResourceGroupDefinition, ResourceGroupMembers,
};
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

struct DirectRows<'source> {
    definitions: Vec<ResourceGroupDefinition<'source>>,
    members: Vec<ResourceGroupMembers>,
}

impl Query<'_> {
    pub(super) fn run_direct<T>(
        self,
        inspect: impl FnOnce(ResourceGroupSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.run_direct_observed(None, inspect)
    }

    pub(super) fn run_direct_observed<T>(
        self,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
        inspect: impl FnOnce(ResourceGroupSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        let Plan {
            owners,
            mut coverage,
        } = Plan::read(
            self.forge,
            Kind::ResourceGroups,
            self.selection,
            self.cancel,
            observer,
        )?;
        let languages = if owners.is_empty() {
            &[]
        } else {
            self.languages
        };
        let sources = TextSources::read_languages(self.archive, languages)?;
        checkpoint(self.cancel)?;
        let origins = CatalogOrigins::new(self.archive, self.entry, &sources)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let texts = Texts::parse(&sources)?;
        let registry = CatalogTextRegistry::default();
        let projection =
            CatalogProjection::new(&texts, &input, &registry).for_kind(Kind::ResourceGroups);
        let rows = project(owners, &projection, &mut coverage, self.cancel)?;
        let table = CatalogTable::new(
            self.forge,
            &[Kind::ResourceGroups.records().type_name],
            rows.definitions,
        );
        let text_rows = projection.finish_rows()?;
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(inspect(ResourceGroupSnapshot {
            build: self.archive.build().cloned(),
            availability: table.availability,
            definitions: table.rows,
            members: rows.members,
            public_names: text_rows.public_names,
            game_texts: text_rows.game_texts,
            requested_languages: self.languages.to_vec(),
            available_languages: languages
                .iter()
                .copied()
                .filter(|language| sources.entry(*language).is_some())
                .collect(),
            localization_notes: texts.notes().to_vec(),
            source_inventory,
            coverage,
        }))
    }
}

fn project<'source>(
    owners: Vec<Owner<'source>>,
    projection: &CatalogProjection<'_, 'source>,
    coverage: &mut CatalogRecordCoverage,
    cancel: Option<&CancelToken>,
) -> Result<DirectRows<'source>, CatalogError> {
    let mut rows = Vec::with_capacity(owners.len());
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(projection.original(), &owner.record)?;
        let definition = crate::resource_group_read::read_definition(
            &owner.tree,
            projection,
            &admission,
            cancel,
        )?;
        let (members, notes) = resource_group_catalog::members(
            &owner.tree,
            projection.original(),
            &admission,
            cancel,
        )?;
        if blocked(&definition.notes) || blocked(&notes) {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push((
            CatalogGuid(owner.record.guid()).text(),
            owner.record.index(),
            definition.value,
            members,
        ));
    }
    rows.sort_by_key(|(guid, physical, _, _)| (*guid, *physical));
    let (definitions, members) = rows
        .into_iter()
        .map(|(_, _, definition, members)| (definition, members))
        .unzip();
    Ok(DirectRows {
        definitions,
        members,
    })
}

fn blocked(notes: &[FieldError]) -> bool {
    notes
        .iter()
        .any(|note| note.description().issue.blocks_projection())
}
