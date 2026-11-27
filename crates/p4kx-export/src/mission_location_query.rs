// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Language, LocalizationNote, Record,
    SourceInventory, TagTreeIssues, TreeBuilder,
};

use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_query_session::{
    CatalogQuerySession, CatalogQuerySource, ParsedCatalogQueries, checkpoint,
};
use crate::catalog_table::CatalogTable;
use crate::catalog_text_contract::{CanonicalText, GameTextRef};
use crate::catalog_v3::tag_catalog::{self, CapturedTags, TagConsumers};
use crate::guid_text::sort_by_guid;
use crate::location_template::{self, MissionLocationTemplate};
use crate::record_observation::RecordObservation;
use crate::record_read::{self, RecordAdmission};
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

const KIND: Kind = Kind::MissionLocationTemplates;

/// The selected canonical mission location templates and their source inventory.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct MissionLocationTemplateSnapshot<'source> {
    /// The installed Archive Build, when available.
    pub build: Option<Build>,
    /// Availability of the mission location template source family.
    pub availability: super::DatasetAvailability,
    /// The selected rows in canonical GUID order with physical ties preserved.
    pub rows: Vec<MissionLocationTemplate<'source>>,
    /// The protected name variants referenced by the selected rows.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// Requested locales, including those absent from the Archive.
    pub requested_languages: Vec<Language>,
    /// Requested locales whose payloads were available and read for this result.
    pub available_languages: Vec<Language>,
    /// Decoding and parsing notes from the localization inputs.
    pub localization_notes: Vec<LocalizationNote>,
    /// Tag graph observations when the query resolved a selected template.
    pub tag_issues: Option<TagTreeIssues>,
    /// The original sources and uses retained by this query.
    pub source_inventory: SourceInventory,
    /// Coverage of Record selection, reading and projection.
    pub coverage: CatalogRecordCoverage,
}

impl<'source> MissionLocationTemplateSnapshot<'source> {
    /// Resolves a protected name among the texts referenced by this result.
    #[must_use]
    pub fn game_text(
        &self,
        reference: &GameTextRef,
    ) -> Option<&CanonicalText<'source, GameTextRef>> {
        self.game_texts
            .iter()
            .find(|text| text.id.value() == Some(reference))
    }
}

/// Supplies the selected authored mission location templates to a callback.
/// Publish consumer-owned changes after this function returns successfully.
///
/// # Errors
/// Returns input, source evidence or cancellation errors.
pub fn with_mission_location_templates<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MissionLocationTemplateSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_mission_location_templates(selection, cancel, inspect)
}

/// Supplies mission location templates for an already decoded entry of the same Archive.
///
/// # Errors
/// Returns the errors of [`with_mission_location_templates`] or an origin mismatch for a foreign entry.
pub fn with_mission_location_templates_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MissionLocationTemplateSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_mission_location_templates(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies mission location templates using this session's Archive and locale policy.
    ///
    /// # Errors
    /// Returns the errors of [`with_mission_location_templates`].
    pub fn with_mission_location_templates<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MissionLocationTemplateSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_mission_location_templates(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies mission location templates using this view's parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_mission_location_templates`].
    pub fn with_mission_location_templates<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MissionLocationTemplateSnapshot<'_>) -> T,
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
    inspect: impl FnOnce(MissionLocationTemplateSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let mut coverage = CatalogRecordCoverage::default();
    let mut records = Vec::new();
    selection.for_each(forge, cancel, |record| {
        match record_read::classify(record, KIND) {
            Ok(RecordAdmission::Selected) => records.push(*record),
            Ok(RecordAdmission::Leftover) => coverage.skipped_leftovers += 1,
            Ok(RecordAdmission::Other) => {}
            Err(_) => coverage.unreadable_headers.push(record.index()),
        }
        Ok(())
    })?;
    checkpoint(cancel)?;
    let selected_languages = if records.is_empty() {
        &[]
    } else {
        requested_languages
    };
    let sources = TextSources::read_languages(archive, selected_languages)?;
    checkpoint(cancel)?;
    let origins = entry.with_texts(archive, &sources)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let texts = Texts::parse(&sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(KIND);
    let tags = (!records.is_empty())
        .then(|| tag_catalog::read(forge, &input, cancel, TagConsumers::default()))
        .transpose()?
        .map(|captured| captured.context);
    let rows = tags.as_ref().map_or_else(
        || Ok(Vec::new()),
        |tags| read_selected(records, &projection, tags, &mut coverage, cancel),
    )?;
    let table = CatalogTable::new(forge, &[KIND.records().type_name], rows);
    let texts_rows = projection.finish_rows()?;
    let available_languages = selected_languages
        .iter()
        .copied()
        .filter(|language| sources.entry(*language).is_some())
        .collect();
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(MissionLocationTemplateSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        game_texts: texts_rows.game_texts,
        requested_languages: requested_languages.to_vec(),
        available_languages,
        localization_notes: texts.notes().to_vec(),
        tag_issues: tags.as_ref().map(|tags| tags.tree().issues()),
        source_inventory,
        coverage,
    }))
}

fn read_selected<'source>(
    records: Vec<Record<'source>>,
    projection: &CatalogProjection<'_, 'source>,
    tags: &CapturedTags<'_, 'source>,
    coverage: &mut CatalogRecordCoverage,
    cancel: Option<&CancelToken>,
) -> Result<Vec<MissionLocationTemplate<'source>>, CatalogError> {
    let mut trees = TreeBuilder::new();
    let mut rows = Vec::new();
    for record in records {
        checkpoint(cancel)?;
        let Ok(tree) = trees.build(&record) else {
            coverage.unreadable_bodies.push(record.index());
            continue;
        };
        let admission = RecordObservation::capture(projection.original(), &record)?;
        match location_template::template(projection, &admission, &tree, tags, cancel) {
            Ok(decoded) => {
                if decoded
                    .notes
                    .iter()
                    .any(|note| note.description().issue.blocks_projection())
                {
                    coverage.incomplete_fields.push(record.index());
                }
                rows.push(decoded.value);
            }
            Err(error) => match error.downcast::<CatalogError>() {
                Ok(error) => return Err(*error),
                Err(_) => coverage.incomplete_fields.push(record.index()),
            },
        }
    }
    sort_by_guid(&mut rows, |row| row.guid);
    Ok(rows)
}
