// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::path::Path;

use p4kx_extraction::{CancelToken, DecimalU64, Language};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::catalog_queries::{ContractOccurrence, ContractSnapshot, ProjectedMissionText};
use crate::catalog_query_session::checkpoint;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_text_contract::GameTextRef;

#[cfg(test)]
#[path = "mission_title_tests.rs"]
mod tests;

const PATH: &str = "projections.missionTitles";

/// Stages the title of each canonical contract in the same physical order.
pub(super) fn collect(
    input: &ContractSnapshot<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<StagedCatalogTable, ReducedError> {
    completeness::inventory(PATH, input.availability, &input.coverage.selected)?;
    if !input.coverage.projection_failures.is_empty()
        || !input.coverage.dependencies.unreadable_headers.is_empty()
        || !input.coverage.dependencies.survey_failures.is_empty()
        || !matches!(
            input.requested_languages.as_slice(),
            [Language::English, Language::German] | [Language::German, Language::English]
        )
    {
        return Err(ReducedError::IncompleteScope(PATH));
    }
    completeness::rows(PATH, input.rows.len(), input.mission_titles.len())?;
    let mut refs = TextRefs::default();
    let mut staged = StagedCatalogRows::new(scratch, cancel);
    for (row, titles) in input.rows.iter().zip(&input.mission_titles) {
        checkpoint(cancel)?;
        let source = &row.base.source;
        let contract = ContractPosition::from(source);
        if source.input_ref() != titles.contract.input_ref()
            || contract != ContractPosition::from(&titles.contract)
        {
            return Err(ReducedError::IncompleteScope(PATH));
        }
        staged.append(&Row {
            contract,
            title: ProjectedText::read(&titles.title, &mut refs)?,
            description: ProjectedText::read(&titles.description, &mut refs)?,
        })?;
    }
    let table = staged.finish(input.availability)?;
    completeness::rows(PATH, input.rows.len(), table.len())?;
    texts.add(refs, &[], &input.game_texts, cancel)?;
    Ok(table)
}

/// Address of a published contract within the selected DataForge.
#[derive(PartialEq, Eq, Serialize)]
pub(super) struct ContractPosition {
    #[serde(rename = "recordIndex")]
    record: u32,
    #[serde(rename = "handlerIndex")]
    handler: DecimalU64,
    #[serde(rename = "contractIndex")]
    contract: DecimalU64,
    #[serde(rename = "subContractIndex")]
    sub_contract: Option<DecimalU64>,
}

impl From<&ContractOccurrence> for ContractPosition {
    fn from(source: &ContractOccurrence) -> Self {
        Self {
            record: source.record_index(),
            handler: DecimalU64::new(source.handler().value_index()),
            contract: DecimalU64::new(source.contract().value_index()),
            sub_contract: source
                .sub_contract()
                .map(|sub| DecimalU64::new(sub.value_index())),
        }
    }
}

#[derive(Serialize)]
struct Row<'read> {
    contract: ContractPosition,
    title: ProjectedText<'read>,
    description: ProjectedText<'read>,
}

#[derive(Serialize)]
struct ProjectedText<'read> {
    key: Cell<&'read GameTextRef>,
    translations: BTreeMap<&'static str, Cell<&'read str>>,
}

impl<'read> ProjectedText<'read> {
    fn read(
        source: &'read ProjectedMissionText<'_>,
        refs: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let key = completeness::cell(PATH, &source.key)?;
        let mut translations = BTreeMap::new();
        for (&language, value) in &source.translations {
            translations.insert(
                language,
                completeness::cell(PATH, value)?.map(AsRef::as_ref),
            );
        }
        refs.game_text(key.value);
        Ok(Self { key, translations })
    }
}
