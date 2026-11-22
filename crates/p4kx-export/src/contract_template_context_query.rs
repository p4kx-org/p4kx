// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Scoped original Template owners and their canonical contextual projections.

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Language, LocalizationNote,
    SourceInventory,
};

use super::unlocalized_record_query::plan::{Owner, Plan};
use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries, checkpoint};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::catalog_text_contract::{CanonicalText, GameTextRef};
use crate::export_diagnostic::ExportSourceRole;
use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::record_observation::RecordObservation;
use crate::template_canonical_read::{self, CanonicalTemplate};
use crate::template_issues::Unknowns;
use crate::template_tag_cache::{DeferredTemplateTags, TemplateTagInput};
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, CatalogReadFailure, Kind};

pub use crate::template_context_value::{LocalizationMeaning, TemplateLocalization};
pub use crate::template_definition::TemplateDefinition;
pub use crate::template_projection::{
    TemplateObjectiveProjection, TemplateProjection, TemplateTagInterpretation,
    TemplateTagProjection,
};
pub use crate::template_source_refs::PhysicalTemplateRef;

/// Canonical context associated with the queried Template Records.
#[non_exhaustive]
pub struct ContractTemplateContextSnapshot<'source> {
    /// The installed Archive Build, when known.
    pub build: Option<Build>,
    /// Availability of the declared Template source family.
    pub availability: DatasetAvailability,
    /// Canonical contextual rows for the selected Template owners.
    pub rows: Vec<TemplateProjection<'source>>,
    /// Original source arenas that contextual row links name.
    pub owners: Vec<TemplateDefinition<'source>>,
    /// Protected text rows registered by the selected inputs.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// Requested locales actually read for these owners.
    pub available_languages: Vec<Language>,
    /// Decoding and parsing notes for the locale inputs.
    pub localization_notes: Vec<LocalizationNote>,
    /// Original inputs and exact uses backing the rows.
    pub source_inventory: SourceInventory,
    /// Physical selection, read omissions, and incomplete contextual fields.
    pub coverage: CatalogRecordCoverage,
    /// Read failures for contextual fields retained on a selected owner.
    pub contextual_failures: Vec<CatalogReadFailure<'source>>,
    /// Whether the selected context required a Tag ancestry survey.
    pub tag_ancestry_surveyed: bool,
    /// Tag source failures, when a selected context required an ancestry survey.
    pub tag_failures: Vec<CatalogReadFailure<'source>>,
}

impl<'source> ContractTemplateContextSnapshot<'source> {
    /// Resolves a contextual row's physical owner and original source arena.
    #[must_use]
    pub fn owner(&self, reference: &PhysicalTemplateRef) -> Option<&TemplateDefinition<'source>> {
        self.owners.iter().find(|owner| {
            owner.input_ref() == reference.input_ref()
                && owner.header().record_index().value() == Some(&reference.record_index())
        })
    }
}

/// Supplies canonical Template context and returns recoverable read omissions in coverage.
///
/// # Errors
/// Returns Archive, accounting, source evidence or cancellation errors.
pub fn with_contract_template_context<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractTemplateContextSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_contract_template_context(selection, cancel, inspect)
}

/// Supplies canonical context using this Archive's already decoded DataForge Entry.
///
/// # Errors
/// Returns query errors or an origin mismatch for a foreign Entry.
pub fn with_contract_template_context_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractTemplateContextSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_contract_template_context(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies canonical Template context through this Archive session.
    ///
    /// # Errors
    /// Returns the errors of [`with_contract_template_context`].
    pub fn with_contract_template_context<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractTemplateContextSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_contract_template_context(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies canonical Template context through the parsed DataForge view.
    ///
    /// # Errors
    /// Returns the errors of [`with_contract_template_context`].
    pub fn with_contract_template_context<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractTemplateContextSnapshot<'_>) -> T,
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
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    languages: &[Language],
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractTemplateContextSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let kind = Kind::ContractTemplates;
    let Plan {
        mut owners,
        mut coverage,
    } = Plan::read_rule(forge, kind.records(), selection, cancel, None)?;
    sort_by_guid(&mut owners, |owner| CatalogGuid(owner.record.guid()));
    let languages = if owners.is_empty() { &[] } else { languages };
    let text_sources = TextSources::read_languages(archive, languages)?;
    checkpoint(cancel)?;
    let origins = CatalogOrigins::new(archive, entry, &text_sources)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let texts = Texts::parse(&text_sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(kind);
    let tags = DeferredTemplateTags::new(forge, &input, cancel);
    let selected = read_rows(
        owners,
        &projection,
        TemplateTagInput::OnDemand(&tags),
        &mut coverage,
        cancel,
    )?;
    let rows = CatalogTable::new(forge, &[kind.records().type_name], selected.projections);
    let text_rows = projection.finish_rows()?;
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    let available_languages = languages
        .iter()
        .copied()
        .filter(|language| text_sources.entry(*language).is_some())
        .collect();
    checkpoint(cancel)?;
    Ok(inspect(ContractTemplateContextSnapshot {
        build: archive.build().cloned(),
        availability: rows.availability,
        rows: rows.rows,
        owners: selected.definitions,
        game_texts: text_rows.game_texts,
        available_languages,
        localization_notes: texts.notes().to_vec(),
        source_inventory,
        coverage,
        contextual_failures: selected.failures,
        tag_ancestry_surveyed: tags.was_read(),
        tag_failures: tags.into_failures(),
    }))
}

#[derive(Default)]
struct Selected<'source> {
    projections: Vec<TemplateProjection<'source>>,
    definitions: Vec<TemplateDefinition<'source>>,
    failures: Vec<CatalogReadFailure<'source>>,
}

fn read_rows<'source>(
    owners: Vec<Owner<'source>>,
    projection: &CatalogProjection<'_, 'source>,
    tags: TemplateTagInput<'_, 'source>,
    coverage: &mut CatalogRecordCoverage,
    cancel: Option<&CancelToken>,
) -> Result<Selected<'source>, CatalogError> {
    let unknowns = Unknowns::default();
    let mut selected = Selected::default();
    let input = projection.original();
    for Owner { record, tree } in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(input, &record)?;
        let original = crate::export_field_walk::inspect_observed(
            Some(input),
            &tree,
            &admission,
            Some(Kind::ContractTemplates),
            ExportSourceRole::Selected,
            cancel,
        )?;
        let CanonicalTemplate {
            definition,
            projection: row,
            ledger,
            failures,
            ..
        } = template_canonical_read::read(projection, tags, &record, &tree, cancel, &unknowns)?;
        if !original.field_projection_complete(0, 0)
            || !ledger.field_projection_complete(0, 0)
            || !failures.is_empty()
        {
            coverage.incomplete_fields.push(record.index());
        }
        selected.projections.push(row);
        selected.definitions.push(definition);
        selected.failures.extend(failures);
    }
    Ok(selected)
}
