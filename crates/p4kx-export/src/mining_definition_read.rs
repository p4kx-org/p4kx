// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{
    Archive, CancelToken, CanonicalReadObserver, DataForge, DataForgeEntry, Language,
};

use super::dependencies::Dependencies;
use super::plan::{Owner, Plan};
use super::rows::Projector;
use super::snapshot::MiningDefinitionSnapshot;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_table::DatasetAvailability;
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

pub(super) fn with_archive<P: Projector, T>(
    archive: &Archive,
    entry: Option<&DataForgeEntry>,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl for<'source> FnOnce(MiningDefinitionSnapshot<'source, P::Row<'source>>) -> T,
) -> Result<T, CatalogError> {
    let session = if let Some(entry) = entry {
        CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
    } else {
        CatalogQuerySession::open(archive, languages, cancel)?
    };
    let parsed = session.parsed(cancel)?;
    parsed.project(cancel, |archive, entry, forge| {
        with_parsed::<P, T>(
            archive,
            entry,
            forge,
            selection,
            session.languages(),
            cancel,
            inspect,
        )
    })
}

pub(super) fn with_parsed<P: Projector, T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    requested_languages: &[Language],
    cancel: Option<&CancelToken>,
    inspect: impl for<'source> FnOnce(MiningDefinitionSnapshot<'source, P::Row<'source>>) -> T,
) -> Result<T, CatalogError> {
    let query = Query {
        archive,
        entry,
        forge,
        selection,
        requested_languages,
        cancel,
    };
    query.run::<P, T>(Observers::default(), inspect)
}

#[derive(Default)]
pub(super) struct Observers {
    pub(super) owners: Option<Arc<dyn CanonicalReadObserver>>,
    pub(super) groups: Option<Arc<dyn CanonicalReadObserver>>,
}

pub(super) struct Query<'query> {
    pub(super) archive: &'query Archive,
    pub(super) entry: &'query DataForgeEntry,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) selection: CatalogRecordSelection,
    pub(super) requested_languages: &'query [Language],
    pub(super) cancel: Option<&'query CancelToken>,
}

impl Query<'_> {
    pub(super) fn run<P: Projector, T>(
        self,
        observers: Observers,
        inspect: impl for<'source> FnOnce(MiningDefinitionSnapshot<'source, P::Row<'source>>) -> T,
    ) -> Result<T, CatalogError> {
        let Self {
            archive,
            entry,
            forge,
            selection,
            requested_languages,
            cancel,
        } = self;
        let Plan {
            owners,
            mut coverage,
        } = Plan::read(forge, P::KIND, selection, cancel, observers.owners)?;
        let selected_languages = if P::LOCALIZED && !owners.is_empty() {
            requested_languages
        } else {
            &[]
        };
        let sources = TextSources::read_languages(archive, selected_languages)?;
        checkpoint(cancel)?;
        let origins = CatalogOrigins::new(archive, entry, &sources)?;
        let input = OriginalCatalogInput::new(forge, &origins);
        let texts = Texts::parse(&sources)?;
        let registry = CatalogTextRegistry::default();
        let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(P::KIND);
        let mut dependencies = Dependencies::new(observers.groups);
        let rows = project_owners::<P>(
            owners,
            &projection,
            &mut coverage,
            &mut dependencies,
            cancel,
        )?;
        let canonical_texts = projection.finish_rows()?;
        let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
        checkpoint(cancel)?;
        Ok(inspect(MiningDefinitionSnapshot {
            build: archive.build().cloned(),
            availability: availability(forge, P::KIND, rows.len()),
            rows,
            public_names: canonical_texts.public_names,
            game_texts: canonical_texts.game_texts,
            requested_languages: requested_languages.to_vec(),
            available_languages: selected_languages
                .iter()
                .copied()
                .filter(|language| sources.entry(*language).is_some())
                .collect(),
            localization_notes: texts.notes().to_vec(),
            source_inventory,
            coverage,
            dependency_coverage: dependencies.coverage,
        }))
    }
}

fn project_owners<'source, P: Projector>(
    owners: Vec<Owner<'source>>,
    projection: &CatalogProjection<'_, 'source>,
    coverage: &mut CatalogRecordCoverage,
    dependencies: &mut Dependencies,
    cancel: Option<&CancelToken>,
) -> Result<Vec<P::Row<'source>>, CatalogError> {
    let mut rows = Vec::new();
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(projection.original(), &owner.record)?;
        let (row, notes) = P::read(&owner.tree, projection, &admission, dependencies, cancel)?;
        if notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push((
            CatalogGuid(owner.record.guid()).text(),
            owner.record.index(),
            row,
        ));
    }
    rows.sort_by_key(|(guid, index, _)| (*guid, *index));
    Ok(rows.into_iter().map(|(_, _, row)| row).collect())
}

fn availability(forge: &DataForge<'_>, kind: Kind, rows: usize) -> DatasetAvailability {
    if rows == 0 {
        DatasetAvailability::from_source_declarations(forge, |name| {
            name == kind.records().type_name
        })
    } else {
        DatasetAvailability::Available
    }
}
