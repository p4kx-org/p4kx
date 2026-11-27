// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{
    Archive, CancelToken, CanonicalReadObserver, DataForge, DataForgeEntry, Filter, Language,
};

use super::rows::{Family, Projector, ReadOptions};
use super::snapshot::EquipmentSnapshot;
use crate::CatalogError;
use crate::Kind;
use crate::build::catalog_queries::unlocalized_record_query::plan::{Owner, Plan};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
};
use crate::catalog_query_session::{CatalogQuerySource, checkpoint};
use crate::catalog_v3::catalog_item_alignment::AlignmentOptions;
use crate::entity_selection::{self, Selected};
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::texts::{TextSources, Texts};

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
    entry: CatalogQuerySource<'_>,
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
    pub(super) entry: CatalogQuerySource<'query>,
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
        self.run_with_item_filter::<P, T>(observer, None, inspect)
    }

    pub(super) fn run_with_item_filter<P: Projector, T>(
        self,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
        item_filter: Option<&Filter>,
        inspect: impl for<'source> FnOnce(EquipmentSnapshot<'source, P::Row<'source>>) -> T,
    ) -> Result<T, CatalogError> {
        let mut plan = self.plan(P::FAMILY, observer, item_filter)?;
        let requested = self.requested(P::FAMILY);
        let selected_languages = if plan.owners.is_empty() {
            &[]
        } else {
            requested
        };
        let item_projection = matches!(P::FAMILY, Family::Items | Family::ItemSources);
        let sources = self.text_sources(P::FAMILY, !plan.owners.is_empty(), selected_languages)?;
        checkpoint(self.cancel)?;
        let origins = self.entry.with_texts(self.archive, &sources)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let texts = Texts::parse(&sources)?;
        let registry = CatalogTextRegistry::default();
        let projection =
            CatalogProjection::new(&texts, &input, &registry).for_kind(P::FAMILY.kind());
        let options = self.read_options(P::FAMILY, item_filter)?;
        let mut rows = project::<P>(
            self.forge,
            plan.owners,
            &projection,
            &options,
            &mut plan.coverage,
            self.cancel,
        )?;
        P::finish(&input, &mut rows, &mut plan.coverage, self.cancel)?;
        let (public_names, game_texts) = (!item_projection)
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

    fn text_sources(
        &self,
        family: Family,
        has_owners: bool,
        languages: &[Language],
    ) -> Result<TextSources, CatalogError> {
        if matches!(family, Family::ItemSources | Family::Items) && has_owners {
            TextSources::read_item_languages(self.archive, &[])
        } else {
            TextSources::read_languages(self.archive, languages)
        }
    }

    fn plan(
        &self,
        family: Family,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
        item_filter: Option<&Filter>,
    ) -> Result<Plan<'query>, CatalogError> {
        checkpoint(self.cancel)?;
        let mut plan = match family {
            Family::MagazineResolutions if item_filter.is_some() => {
                self.plan_filtered_magazines(item_filter, observer)?
            }
            Family::Definitions
            | Family::Items
            | Family::ItemSources
            | Family::InventoryContainers
            | Family::CargoLoadingGlobals
            | Family::ShopCommodityGlobals
            | Family::AmmoDefinitions
            | Family::ExplosionDefinitions
            | Family::ResourceNetworkGlobals
            | Family::ProceduralPlanetAudio
            | Family::Factions
            | Family::ContractScopes
            | Family::WeaponSources
            | Family::ProjectileAmmo
            | Family::MagazineResolutions => Plan::read(
                self.forge,
                family.kind(),
                self.selection,
                self.cancel,
                observer,
            )?,
            Family::Configuration(family) => Plan::read_entity_components(
                self.forge,
                family.source_types(),
                self.selection,
                self.cancel,
                observer,
            )?,
            Family::ItemEquipment(family) => Plan::read_item_components(
                self.forge,
                family.source_types(),
                self.selection,
                self.cancel,
                observer,
            )?,
            Family::Attachments
            | Family::ItemPorts
            | Family::DeclaredEquipment
            | Family::Bounds
            | Family::CargoGrids
            | Family::Facilities => {
                Plan::read_entities(self.forge, self.selection, self.cancel, observer)?
            }
        };
        order(family, &mut plan.owners);
        Ok(plan)
    }

    fn plan_filtered_magazines(
        &self,
        item_filter: Option<&Filter>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
    ) -> Result<Plan<'query>, CatalogError> {
        Plan::read_with(
            self.forge,
            self.selection,
            self.cancel,
            observer,
            |record, _, coverage| {
                match entity_selection::select(record, item_filter) {
                    Err(_) => coverage.unreadable_headers.push(record.index()),
                    Ok(Some(Selected::Leftover(Kind::Items))) => {
                        coverage.skipped_leftovers += 1;
                    }
                    Ok(Some(Selected::Entity(Kind::Items))) => return Ok(true),
                    Ok(_) => {}
                }
                Ok(false)
            },
        )
    }

    pub(super) fn read_options<'options>(
        &'options self,
        family: Family,
        item_filter: Option<&'options Filter>,
    ) -> Result<ReadOptions<'options>, CatalogError> {
        Ok(ReadOptions {
            alignment: if matches!(
                family,
                Family::Attachments | Family::ItemPorts | Family::DeclaredEquipment
            ) {
                AlignmentOptions::read(self.forge, self.cancel)?
            } else {
                AlignmentOptions::default()
            },
            item_filter,
        })
    }

    pub(super) const fn requested(&self, family: Family) -> &[Language] {
        if family.localized() {
            self.languages
        } else {
            &[]
        }
    }
}

fn order(family: Family, owners: &mut [Owner<'_>]) {
    match family {
        Family::Definitions
        | Family::Items
        | Family::ItemSources
        | Family::AmmoDefinitions
        | Family::ExplosionDefinitions
        | Family::ResourceNetworkGlobals
        | Family::ProceduralPlanetAudio
        | Family::Factions
        | Family::ContractScopes
        | Family::InventoryContainers
        | Family::CargoLoadingGlobals
        | Family::ShopCommodityGlobals => owners.sort_by_cached_key(|owner| {
            (
                CatalogGuid(owner.record.guid()).text(),
                owner.record.index(),
            )
        }),
        Family::Attachments
        | Family::ItemPorts
        | Family::DeclaredEquipment
        | Family::Bounds
        | Family::CargoGrids
        | Family::Facilities
        | Family::Configuration(_)
        | Family::ItemEquipment(_)
        | Family::WeaponSources
        | Family::ProjectileAmmo
        | Family::MagazineResolutions => owners.sort_by_key(|owner| owner.record.index()),
    }
}

fn project<'source, P: Projector>(
    forge: &'source DataForge<'source>,
    owners: Vec<Owner<'source>>,
    projection: &CatalogProjection<'_, 'source>,
    options: &ReadOptions<'_>,
    coverage: &mut CatalogRecordCoverage,
    cancel: Option<&CancelToken>,
) -> Result<Vec<P::Row<'source>>, CatalogError> {
    let mut rows = Vec::new();
    for owner in owners {
        rows.extend(project_owner::<P>(
            forge, &owner, projection, options, coverage, cancel,
        )?);
    }
    Ok(rows)
}

pub(super) fn project_owner<'source, P: Projector>(
    forge: &'source DataForge<'source>,
    owner: &Owner<'source>,
    projection: &CatalogProjection<'_, 'source>,
    options: &ReadOptions<'_>,
    coverage: &mut CatalogRecordCoverage,
    cancel: Option<&CancelToken>,
) -> Result<Vec<P::Row<'source>>, CatalogError> {
    checkpoint(cancel)?;
    let admission = RecordObservation::capture(projection.original(), &owner.record)?;
    let read = P::read(forge, &owner.tree, projection, &admission, options, cancel)?;
    if !read.selection_complete
        || read
            .notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
    {
        coverage.incomplete_fields.push(owner.record.index());
    }
    Ok(read.value)
}
