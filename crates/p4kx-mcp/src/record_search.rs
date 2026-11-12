// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{DataForge, DataForgeOrigin, Filter, Record};
use rayon::prelude::*;
use serde::Serialize;

use crate::budget::{fill_within, fit_prefix, json_len};
use crate::error::ToolError;
use crate::forge_shapes::{RecordSearchResult, RecordShape};
use crate::page::Page;
use crate::record_search_origins::{self, Navigation, Survey};

/// Returns matching Records in the DataForge path-index order.
pub(crate) fn search(
    forge: &DataForge<'_>,
    archive: &str,
    origin: &DataForgeOrigin,
    (path, type_name): (Option<&str>, Option<&str>),
    page: Page,
) -> Result<RecordSearchResult, ToolError> {
    let path = path.map(Filter::parse).transpose()?;
    let type_name = type_name.map(Filter::parse).transpose()?;
    let (matches, survey) = select(forge, path.as_ref(), type_name.as_ref());
    let total = matches.len();
    let items = page
        .slice(&matches)
        .iter()
        .filter_map(|&index| forge.record(index))
        .map(|record| Row::of(&record));
    let reserved = json_len(&(archive, total));
    let (rows, _) = fill_within(page, total, reserved, items, |record| {
        format!("Record {}", record.shape.name)
    })?;
    let render = |navigation| {
        fit_prefix(
            rows.len(),
            |len| {
                let next_cursor = page.next_cursor_after(len, total)?;
                let selected = rows.split_at(len).0;
                let origins = record_search_origins::project(
                    origin,
                    &survey,
                    selected,
                    next_cursor.is_some(),
                    navigation,
                )?;
                Ok(RecordSearchResult {
                    archive: archive.to_owned(),
                    total,
                    records: selected.iter().map(|row| &row.shape).collect::<Vec<_>>(),
                    next_cursor,
                    origins,
                })
            },
            || format!("Record at search position {}", page.offset()),
        )
    };
    let (keep, result) = match render(Navigation::Complete) {
        Err(ToolError::TooLargeItem { .. }) => render(Navigation::HeaderWitness)?,
        result => result?,
    };
    let RecordSearchResult {
        archive,
        total,
        next_cursor,
        origins,
        ..
    } = result;
    Ok(RecordSearchResult {
        archive,
        total,
        records: rows.into_iter().take(keep).map(|row| row.shape).collect(),
        next_cursor,
        origins,
    })
}

#[derive(Serialize)]
pub(crate) struct Row {
    #[serde(flatten)]
    shape: RecordShape,
    #[serde(skip)]
    pub(crate) record: u32,
    #[serde(skip)]
    pub(crate) structure: u32,
}

impl Row {
    fn of(record: &Record<'_>) -> Result<Self, ToolError> {
        let structure = record.struct_type()?;
        Ok(Self {
            shape: RecordShape {
                guid: record.guid().to_string(),
                name: record.name()?.to_owned(),
                type_name: structure.name()?.to_owned(),
                path: record.record_path()?.to_owned(),
            },
            record: record.index(),
            structure: structure.index(),
        })
    }
}

struct TypeMatch {
    definition: u32,
    matches: bool,
}

fn select(
    forge: &DataForge<'_>,
    path: Option<&Filter>,
    type_name: Option<&Filter>,
) -> (Vec<u32>, Survey) {
    let order = forge.path_order();
    let eligible = order.par_iter().filter_map(|&index| {
        let record = forge.record(index)?;
        path.is_none_or(|filter| record.record_path().is_ok_and(|text| filter.matches(text)))
            .then_some(record)
    });
    let mut type_records = Vec::new();
    let mut type_definitions = BTreeSet::new();
    let mut complete = true;
    let matches = if let Some(filter) = type_name {
        let examined: Vec<_> = eligible
            .map(|record| (record.index(), test_type(&record, filter)))
            .collect();
        let mut matches = Vec::new();
        for (index, check) in examined {
            if let Some(check) = check {
                type_records.push(index);
                type_definitions.insert(check.definition);
                if check.matches {
                    matches.push(index);
                }
            } else {
                complete = false;
            }
        }
        matches
    } else {
        eligible.map(|record| record.index()).collect()
    };
    let survey = Survey::of(
        forge.records().len(),
        order,
        type_records,
        type_definitions,
        complete,
    );
    (matches, survey)
}

fn test_type(record: &Record<'_>, filter: &Filter) -> Option<TypeMatch> {
    let definition = record.struct_type().ok()?;
    let name = definition.name().ok()?;
    Some(TypeMatch {
        definition: definition.index(),
        matches: filter.matches(name),
    })
}
