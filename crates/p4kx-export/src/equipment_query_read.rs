// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{
    Archive, CancelToken, CanonicalReadObserver, DataForge, DataForgeEntry, Language,
};

use super::rows::{Family, Projector};
use super::snapshot::EquipmentSnapshot;
use crate::build::catalog_queries::unlocalized_record_query::plan::{Owner, Plan};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::catalog_item_alignment::AlignmentOptions;
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
    inspect: impl for<'source> FnOnce(EquipmentSnapshot<'source, P::Row<'source>>) -> T,
) -> Result<T, CatalogError> {
    let session = match entry {
        Some(entry) => CatalogQuerySession::from_entry(archive, entry, languages, cancel)?,
        None => CatalogQuerySession::open(archive, languages, cancel)?,
    };
    session
        .parsed(cancel)?
        .project(cancel, |archive, entry, forge| {
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
    languages: &[Language],
    cancel: Option<&CancelToken>,
    inspect: impl for<'source> FnOnce(EquipmentSnapshot<'source, P::Row<'source>>) -> T,
) -> Result<T, CatalogError> {
    Query {
        archive,
        entry,
        forge,
        selection,
        languages,
        cancel,
    }
    .run::<P, T>(None, inspect)
}

pub(super) struct Query<'query> {
    pub(super) archive: &'query Archive,
    pub(super) entry: &'query DataForgeEntry,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) selection: CatalogRecordSelection,
    pub(super) languages: &'query [Language],
    pub(super) cancel: Option<&'query CancelToken>,
}

impl<'query> Query<'query> {
    pub(super) fn run<P: Projector, T>(
        self,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
        inspect: impl for<'source> FnOnce(EquipmentSnapshot<'source, P::Row<'source>>) -> T,
    ) -> Result<T, CatalogError> {
        let mut plan = self.plan(P::FAMILY, observer)?;
        let requested = self.requested(P::FAMILY);
        let selected_languages = if plan.owners.is_empty() {
            &[]
        } else {
            requested
        };
        let sources = if matches!(P::FAMILY, Family::ItemSources) && !plan.owners.is_empty() {
            TextSources::read_item_languages(self.archive, &[])?
        } else {
            TextSources::read_languages(self.archive, selected_languages)?
        };
        checkpoint(self.cancel)?;
        let origins = CatalogOrigins::new(self.archive, self.entry, &sources)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let texts = Texts::parse(&sources)?;
        let registry = CatalogTextRegistry::default();
        let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(Kind::Items);
        let options = if matches!(P::FAMILY, Family::Attachments) {
            AlignmentOptions::read(self.forge, self.cancel)?
        } else {
            AlignmentOptions::default()
        };
        let rows = project::<P>(
            plan.owners,
            &projection,
            &options,
            &mut plan.coverage,
            self.cancel,
        )?;
        let (public_names, game_texts) = (!matches!(P::FAMILY, Family::ItemSources))
            .then(|| projection.finish_rows())
            .transpose()?
            .map_or_else(Default::default, |rows| {
                (rows.public_names, rows.game_texts)
            });
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(inspect(EquipmentSnapshot {
            build: self.archive.build().cloned(),
            availability: P::FAMILY.availability(self.forge, rows.len()),
            rows,
            public_names,
            game_texts,
            requested_languages: requested.to_vec(),
            available_languages: selected_languages
                .iter()
                .copied()
                .filter(|language| sources.entry(*language).is_some())
                .collect(),
            localization_notes: texts.notes().to_vec(),
            source_inventory,
            coverage: plan.coverage,
            dependency_coverage: (),
        }))
    }

    fn plan(
        &self,
        family: Family,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
    ) -> Result<Plan<'query>, CatalogError> {
        checkpoint(self.cancel)?;
        let mut plan = match family {
            Family::Definitions | Family::ItemSources => Plan::read(
                self.forge,
                Kind::Items,
                self.selection,
                self.cancel,
                observer,
            )?,
            Family::Attachments | Family::Bounds => {
                Plan::read_entities(self.forge, self.selection, self.cancel, observer)?
            }
        };
        order(family, &mut plan.owners);
        Ok(plan)
    }

    const fn requested(&self, family: Family) -> &[Language] {
        if family.localized() {
            self.languages
        } else {
            &[]
        }
    }
}

fn order(family: Family, owners: &mut [Owner<'_>]) {
    match family {
        Family::Definitions | Family::ItemSources => owners.sort_by_key(|owner| {
            (
                CatalogGuid(owner.record.guid()).text(),
                owner.record.index(),
            )
        }),
        Family::Attachments | Family::Bounds => owners.sort_by_key(|owner| owner.record.index()),
    }
}

fn project<'source, P: Projector>(
    owners: Vec<Owner<'source>>,
    projection: &CatalogProjection<'_, 'source>,
    options: &AlignmentOptions<'_>,
    coverage: &mut CatalogRecordCoverage,
    cancel: Option<&CancelToken>,
) -> Result<Vec<P::Row<'source>>, CatalogError> {
    let mut rows = Vec::new();
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(projection.original(), &owner.record)?;
        let read = P::read(&owner.tree, projection, &admission, options, cancel)?;
        if !read.selection_complete
            || read
                .notes
                .iter()
                .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.extend(read.value);
    }
    Ok(rows)
}
