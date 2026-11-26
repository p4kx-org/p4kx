// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourceValueState, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::record_identity::RecordIdentity;
use super::record_targets::RecordTargets;
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::catalog_queries::{
    CatalogRecordSelection, ManufacturerDefinition, ParsedCatalogQueries,
};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};

pub(super) const PATH: &str = "definitions.manufacturers";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<(StagedCatalogTable, RecordTargets), ReducedError> {
    parsed.with_manufacturers(CatalogRecordSelection::All, cancel, |snapshot| {
        completeness::inventory(PATH, snapshot.availability, &snapshot.coverage)?;
        let mut definitions = StagedCatalogRows::new(scratch, cancel);
        let mut references = TextRefs::default();
        let mut targets = RecordTargets::default();
        for definition in &snapshot.rows {
            targets.insert(PATH, definition.source())?;
            definitions.append(&Row::project(definition, &mut references)?)?;
        }
        texts.add(
            references,
            &snapshot.public_names,
            &snapshot.game_texts,
            cancel,
        )?;
        let table = definitions.finish(snapshot.availability)?;
        completeness::rows(PATH, snapshot.rows.len(), table.len())?;
        Ok((table, targets))
    })?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row<'source> {
    #[serde(flatten)]
    identity: RecordIdentity<'source>,
    code: Cell<&'source str>,
    localization: Cell<Localization<'source>>,
}

impl<'source> Row<'source> {
    fn project(
        definition: &'source ManufacturerDefinition<'source>,
        texts: &mut TextRefs<'source>,
    ) -> Result<Self, ReducedError> {
        selected_cell(definition.code())?;
        selected_cell(definition.localization())?;
        let localization = Cell::from(definition.localization()).try_map(|row| {
            selected_cell(row.name())?;
            selected_cell(row.short_name())?;
            selected_cell(row.description())?;
            texts.name(row.name());
            texts.name(row.short_name());
            texts.description(row.description());
            Ok::<_, ReducedError>(Localization {
                name: row.name().into(),
                short_name: row.short_name().into(),
                description: row.description().into(),
            })
        })?;
        Ok(Self {
            identity: definition.source().into(),
            code: Cell::from(definition.code()).map(|code| *code),
            localization,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Localization<'source> {
    name: Cell<&'source PublicNameRef>,
    short_name: Cell<&'source PublicNameRef>,
    description: Cell<&'source GameTextRef>,
}

fn selected_cell<T>(cell: &SourcedValue<T>) -> Result<(), ReducedError> {
    if cell.origin().source_set_complete()
        && matches!(
            cell.state(),
            SourceValueState::Present | SourceValueState::Absent | SourceValueState::Null
        )
    {
        Ok(())
    } else {
        Err(ReducedError::IncompleteScope(PATH))
    }
}
