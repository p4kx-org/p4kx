// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, Language};

use super::super::unlocalized_record_query::plan::{Owner, Plan};
use super::super::{CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability};
use super::ItemRecoverySnapshot;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_query_session::{CatalogQuerySource, checkpoint};
use crate::record_observation::RecordObservation;
use crate::recovery::{self, RECOVERY, RecoveryProjection};
use crate::texts::{TextSources, Texts};

pub(super) struct Query<'query> {
    pub(super) archive: &'query Archive,
    pub(super) entry: CatalogQuerySource<'query>,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) selection: CatalogRecordSelection,
    pub(super) requested_languages: &'query [Language],
    pub(super) cancel: Option<&'query CancelToken>,
}

impl<'query> Query<'query> {
    pub(super) fn run<T>(
        self,
        inspect: impl FnOnce(ItemRecoverySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        let (selected, mut coverage) = self.select()?;
        let languages = if selected.is_some() {
            self.requested_languages
        } else {
            &[]
        };
        checkpoint(self.cancel)?;
        let sources = TextSources::read_languages(self.archive, languages)?;
        checkpoint(self.cancel)?;
        let origins = self.entry.with_texts(self.archive, &sources)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let texts = Texts::parse(&sources)?;
        let registry = CatalogTextRegistry::default();
        let projection = CatalogProjection::new(&texts, &input, &registry);
        let projected = selected
            .map(|owner| project(&owner, &projection, &mut coverage, self.cancel))
            .transpose()?;
        let availability = self.availability(projected.is_some());
        let (economy, overrides, policy) = projected.map_or((None, None, None), |rows| {
            (Some(rows.economy), Some(rows.overrides), Some(rows.policy))
        });
        let game_texts = projection.finish_rows()?.game_texts;
        let available_languages = languages
            .iter()
            .copied()
            .filter(|language| sources.entry(*language).is_some())
            .collect();
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(inspect(ItemRecoverySnapshot {
            build: self.archive.build().cloned(),
            availability,
            economy,
            overrides,
            policy,
            game_texts,
            requested_languages: self.requested_languages.to_vec(),
            available_languages,
            localization_notes: texts.notes().to_vec(),
            source_inventory,
            coverage,
        }))
    }

    fn select(&self) -> Result<(Option<Owner<'query>>, CatalogRecordCoverage), CatalogError> {
        let Plan { owners, coverage } =
            Plan::read_rule(self.forge, RECOVERY, self.selection, self.cancel, None)?;
        let selected = owners
            .into_iter()
            .min_by_key(|owner| recovery::precedence(&owner.tree));
        Ok((selected, coverage))
    }

    fn availability(&self, projected: bool) -> DatasetAvailability {
        if projected {
            DatasetAvailability::Available
        } else {
            DatasetAvailability::from_source_declarations(self.forge, |name| {
                name == RECOVERY.type_name
            })
        }
    }
}

fn project<'source>(
    owner: &Owner<'source>,
    projection: &CatalogProjection<'_, 'source>,
    coverage: &mut CatalogRecordCoverage,
    cancel: Option<&CancelToken>,
) -> Result<RecoveryProjection<'source>, CatalogError> {
    let admission = RecordObservation::capture(projection.original(), &owner.record)?;
    let projected = recovery::project_policy(&owner.tree, projection, &admission, cancel)?;
    if projected
        .notes
        .iter()
        .any(|note| note.description().issue.blocks_projection())
    {
        coverage.incomplete_fields.push(owner.record.index());
    }
    Ok(projected)
}
