// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ENTITY_TYPE;
use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Language, LocalizationNote,
    PointerProbe, Record, SourceInventory, TreeBuilder,
};

use super::{
    CatalogLanguageSelection, CatalogPublicName, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ItemPortContainer,
};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_query_session::{
    CatalogQuerySession, CatalogQuerySource, ParsedCatalogQueries, checkpoint,
};
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::catalog_entity_configurations;
use crate::catalog_v3::catalog_item_alignment::{AlignmentOptions, CONTAINER};
use crate::entity_selection::{self, Selected};
use crate::record_observation::RecordObservation;
use crate::scan::{CRAFTER_FIELD, CRAFTER_STRUCT};
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

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
    checkpoint(cancel)?;
    let sources = TextSources::read_languages(archive, requested_languages)?;
    checkpoint(cancel)?;
    let origins = entry.with_texts(archive, &sources)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let texts = Texts::parse(&sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry);
    let options = AlignmentOptions::read(forge, cancel)?;
    let crafter = forge.pointer_probe(CRAFTER_FIELD, CRAFTER_STRUCT);
    let mut trees = TreeBuilder::new();
    let mut state = SelectedPorts::default();
    selection.for_each(forge, cancel, |record| {
        state.read(
            record,
            &projection,
            &options,
            crafter.as_ref(),
            &mut trees,
            cancel,
        )
    })?;
    let table = CatalogTable::new(forge, &[CONTAINER], state.rows);
    let public_names = projection
        .finish_public_names()?
        .into_iter()
        .map(CatalogPublicName::from)
        .collect();
    let available_languages = requested_languages
        .iter()
        .copied()
        .filter(|language| sources.entry(*language).is_some())
        .collect();
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    Ok(inspect(ItemPortSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        public_names,
        requested_languages: requested_languages.to_vec(),
        available_languages,
        localization_notes: texts.notes().to_vec(),
        source_inventory,
        coverage: state.coverage,
    }))
}

#[derive(Default)]
struct SelectedPorts<'source> {
    rows: Vec<ItemPortContainer<'source>>,
    coverage: CatalogRecordCoverage,
}

impl<'source> SelectedPorts<'source> {
    fn read(
        &mut self,
        record: &Record<'source>,
        projection: &CatalogProjection<'_, 'source>,
        options: &AlignmentOptions<'_>,
        crafter: Option<&PointerProbe<'source>>,
        trees: &mut TreeBuilder<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let selection = record.type_name().and_then(|source_type| {
            if source_type == ENTITY_TYPE {
                entity_selection::select(record, None)
            } else {
                Ok(None)
            }
        });
        match selection {
            Err(_) => self.coverage.unreadable_headers.push(record.index()),
            Ok(Some(Selected::Leftover(_))) => self.coverage.skipped_leftovers += 1,
            Ok(Some(Selected::Entity(kind))) => {
                checkpoint(cancel)?;
                if kind == Kind::Crafters {
                    match crafter.map_or(Ok(false), |probe| trees.holds(probe, record)) {
                        Ok(true) => {}
                        Ok(false) => return Ok(()),
                        Err(_) => {
                            self.coverage.unreadable_headers.push(record.index());
                            return Ok(());
                        }
                    }
                }
                let Ok(tree) = trees.build(record) else {
                    self.coverage.unreadable_bodies.push(record.index());
                    return Ok(());
                };
                let admission = RecordObservation::capture(projection.original(), record)?;
                let (rows, notes) = catalog_entity_configurations::read_item_ports(
                    &tree, projection, &admission, options, cancel,
                )?;
                if notes
                    .iter()
                    .any(|note| note.description().issue.blocks_projection())
                {
                    self.coverage.incomplete_fields.push(record.index());
                }
                self.rows.extend(rows);
            }
            Ok(None) => {}
        }
        Ok(())
    }
}
