// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::record_identity::RecordIdentity;
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::catalog_queries::equipment::contract_scopes::{
    ContractScopeGlobalParams, ContractScopeItem,
};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_text_contract::PublicNameRef;

const PATH: &str = "definitions.contractScopes";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<StagedCatalogTable, ReducedError> {
    parsed.with_contract_scopes(CatalogRecordSelection::All, cancel, |input| {
        completeness::inventory(PATH, input.availability(), input.coverage())?;
        let mut references = TextRefs::default();
        let mut staged = StagedCatalogRows::new(scratch, cancel);
        for row in input.rows() {
            staged.append(&Row::read(row, &mut references)?)?;
        }
        let table = staged.finish(input.availability())?;
        completeness::rows(PATH, input.rows().len(), table.len())?;
        texts.add(references, input.public_names(), input.game_texts(), cancel)?;
        Ok(table)
    })?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    location: Cell<Label<'read>>,
    cluster: Cell<Label<'read>>,
    sector: Cell<Label<'read>>,
    planetary_body: Cell<Label<'read>>,
    planetary_system: Cell<Label<'read>>,
    solar_system: Cell<Label<'read>>,
    universe: Cell<Label<'read>>,
}

impl<'read> Row<'read> {
    fn read(
        value: &'read ContractScopeGlobalParams<'_>,
        texts: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let mut scope = |slot| label(slot, texts);
        Ok(Self {
            identity: value.source().into(),
            location: scope(value.location())?,
            cluster: scope(value.cluster())?,
            sector: scope(value.sector())?,
            planetary_body: scope(value.planetary_body())?,
            planetary_system: scope(value.planetary_system())?,
            solar_system: scope(value.solar_system())?,
            universe: scope(value.universe())?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Label<'read> {
    display_name: Cell<&'read PublicNameRef>,
}

fn label<'read>(
    slot: &'read SourcedValue<ContractScopeItem<'_>>,
    texts: &mut TextRefs<'read>,
) -> Result<Cell<Label<'read>>, ReducedError> {
    completeness::value(PATH, slot)?;
    Cell::from(slot).try_map(|item| {
        let name = item.display_name();
        completeness::value(PATH, name)?;
        texts.name(name);
        Ok(Label {
            display_name: name.into(),
        })
    })
}
