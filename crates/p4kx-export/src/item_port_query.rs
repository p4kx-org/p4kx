// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::OnceCell;

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Language, LocalizationNote,
    SourceInventory,
};

use super::{
    CatalogLanguageSelection, CatalogPublicName, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ItemPortContainer,
};
use crate::CatalogError;
use crate::build::catalog_queries::unlocalized_record_query::plan::{Owner, Plan};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_query_session::{
    CatalogQuerySession, CatalogQuerySource, ParsedCatalogQueries, checkpoint,
};
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::catalog_entity_configurations;
use crate::catalog_v3::catalog_item_alignment::{AlignmentOptions, CONTAINER};
use crate::record_observation::RecordObservation;
use crate::texts::{TextSources, Texts};

/// The selected original port declarations and their source inventory.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ItemPortSnapshot<'source> {
    /// The installed Archive Build, when available.
    pub build: Option<Build>,
    /// Availability of the original port-container family.
    pub availability: DatasetAvailability,
    /// Containers in physical Record and component order.
    pub rows: Vec<ItemPortContainer<'source>>,
    /// Public labels referenced by these rows.
    pub public_names: Vec<CatalogPublicName>,
    /// Requested locales, including those absent from the Archive.
    pub requested_languages: Vec<Language>,
    /// Requested locales with an available Archive payload.
    pub available_languages: Vec<Language>,
    /// Lossy decoding and skipped lines from the requested locale inputs.
    pub localization_notes: Vec<LocalizationNote>,
    /// Sources and uses referenced by the selected rows.
    pub source_inventory: SourceInventory,
    /// Coverage of Record selection, reads and projections.
    pub coverage: CatalogRecordCoverage,
}

/// Supplies original item ports and their evidence from selected Records to a callback.
/// Commit caller-owned changes only after this function returns successfully.
///
/// # Errors
/// Returns input, source evidence or cancellation errors.
pub fn with_item_ports<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ItemPortSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    with_archive(archive, None, selection, languages, cancel, inspect)
}

/// Supplies item ports using a previously decoded entry from the same Archive.
///
/// # Errors
/// Returns the errors of [`with_item_ports`], or an origin mismatch for a foreign entry.
pub fn with_item_ports_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ItemPortSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    with_archive(archive, Some(entry), selection, languages, cancel, inspect)
}

fn with_archive<T>(
    archive: &Archive,
    entry: Option<&DataForgeEntry>,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ItemPortSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let session = match entry {
        Some(entry) => CatalogQuerySession::from_entry(archive, entry, languages, cancel)?,
        None => CatalogQuerySession::open(archive, languages, cancel)?,
    };
    session.with_item_ports(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected item-port declarations using this session's Archive and locale policy.
    ///
    /// # Errors
    /// Returns the errors of [`with_item_ports`].
    pub fn with_item_ports<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ItemPortSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_item_ports(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected item-port declarations using this view's parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_item_ports`].
    pub fn with_item_ports<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ItemPortSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            with_parsed(
                archive,
                entry,
                forge,
                selection,
                self.languages(),
                cancel,
                inspect,
            )
        })
    }
}

fn with_parsed<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    requested_languages: &[Language],
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ItemPortSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let sources = OnceCell::new();
    let projection = OnceCell::new();
    let mut rows = Vec::new();
    let coverage =
        Plan::visit_entities::<CatalogError>(forge, selection, None, cancel, |owner, coverage| {
            let inputs = if let Some(inputs) = projection.get() {
                inputs
            } else {
                let loaded = TextSources::read_languages(archive, requested_languages)?;
                let sources = sources.get_or_init(|| loaded);
                let inputs = PortProjection::read(archive, entry, forge, sources, cancel)?;
                projection.get_or_init(|| inputs)
            };
            inputs.project(forge, &owner, &mut rows, coverage, cancel)
        })?;
    let table = CatalogTable::new(forge, &[CONTAINER], rows);
    let (public_names, localization_notes, source_inventory) = match projection.get() {
        Some(inputs) => inputs.finish(forge, cancel)?,
        None => (
            Vec::new(),
            Vec::new(),
            entry.registry().finish_with(|| checkpoint(cancel))?,
        ),
    };
    checkpoint(cancel)?;
    Ok(inspect(ItemPortSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        requested_languages: requested_languages.to_vec(),
        available_languages: requested_languages
            .iter()
            .copied()
            .filter(|language| {
                sources
                    .get()
                    .is_some_and(|sources| sources.entry(*language).is_some())
            })
            .collect(),
        localization_notes,
        public_names,
        source_inventory,
        coverage,
    }))
}

struct PortProjection<'source> {
    origins: CatalogOrigins,
    texts: Texts<'source>,
    registry: CatalogTextRegistry<'source>,
    options: AlignmentOptions<'source>,
}

impl<'source> PortProjection<'source> {
    fn read(
        archive: &Archive,
        entry: CatalogQuerySource<'_>,
        forge: &'source DataForge<'_>,
        sources: &'source TextSources,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        checkpoint(cancel)?;
        Ok(Self {
            origins: entry.with_texts(archive, sources)?,
            texts: Texts::parse(sources)?,
            registry: CatalogTextRegistry::default(),
            options: AlignmentOptions::read(forge, cancel)?,
        })
    }

    fn project(
        &self,
        forge: &DataForge<'source>,
        owner: &Owner<'source>,
        rows: &mut Vec<ItemPortContainer<'source>>,
        coverage: &mut CatalogRecordCoverage,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        checkpoint(cancel)?;
        let input = OriginalCatalogInput::new(forge, &self.origins);
        let projection = CatalogProjection::new(&self.texts, &input, &self.registry);
        let admission = RecordObservation::capture(&input, &owner.record)?;
        let (selected, notes) = catalog_entity_configurations::read_item_ports(
            &owner.tree,
            &projection,
            &admission,
            &self.options,
            cancel,
        )?;
        if notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.extend(selected);
        Ok(())
    }

    fn finish(
        &self,
        forge: &DataForge<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<
        (
            Vec<CatalogPublicName>,
            Vec<LocalizationNote>,
            SourceInventory,
        ),
        CatalogError,
    > {
        let input = OriginalCatalogInput::new(forge, &self.origins);
        let projection = CatalogProjection::new(&self.texts, &input, &self.registry);
        let names = projection
            .finish_public_names()?
            .into_iter()
            .map(CatalogPublicName::from)
            .collect();
        let inventory = self.origins.uses.finish_with(|| checkpoint(cancel))?;
        Ok((names, self.texts.notes().to_vec(), inventory))
    }
}
