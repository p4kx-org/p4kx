// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice;

use p4kx_extraction::query::{CompiledQuery, RECORD_PATH_COLUMN, Row};
use p4kx_extraction::{DataForgeOrigin, FieldPath, SourceUse, SourceUseRole, Value};

use crate::forge_origins::{EvidenceError, ForgeOrigins, site};
use crate::forge_shapes::ForgeFieldOrigins;
use crate::query_selection_sources::{Survey, SurveyMode};
use crate::query_source_uses::{record_site, uses};

pub(crate) struct PageEvidence<'a, 'source> {
    pub(crate) origin: &'a DataForgeOrigin,
    pub(crate) compiled: &'a CompiledQuery<'source>,
    pub(crate) rows: &'a [Row<'source>],
    pub(crate) examined: &'a [Row<'source>],
    pub(crate) columns: &'a [usize],
    pub(crate) survey: &'a Survey,
    pub(crate) mode: SurveyMode,
    pub(crate) cursor: bool,
}

pub(crate) fn project(page: &PageEvidence<'_, '_>) -> Result<ForgeFieldOrigins, EvidenceError> {
    let mut origins = ForgeOrigins::new(page.origin)?;
    for local in ["archive", "query"] {
        origins.put(local, origins.uses.origin(&[], true))?;
    }
    columns(&mut origins, page.compiled)?;
    navigation(&mut origins, page)?;
    rows(&mut origins, page)?;
    origins.finish()
}

fn columns(origins: &mut ForgeOrigins, compiled: &CompiledQuery<'_>) -> Result<(), EvidenceError> {
    let mut all = Vec::new();
    for (column, kind) in compiled.column_kinds().iter().enumerate() {
        let at = format!("columns/{column}");
        for field in ["name", "list"] {
            origins.put(&format!("{at}/{field}"), origins.uses.origin(&[], true))?;
        }
        let ranges = ordinal_ranges(compiled.column_structs(column));
        let uses = if ranges.is_empty() {
            Vec::new()
        } else {
            vec![site(
                &origins.scope,
                "dataforgeStructDefinition",
                "columnLayouts",
                ranges,
                SourceUseRole::Conversion,
            )?]
        };
        bind(
            origins,
            &format!("{at}/types"),
            &uses,
            kind.data_types().is_empty() || origins.complete,
        )?;
        for ordinal in 0..kind.data_types().len() {
            bind(
                origins,
                &format!("{at}/types/{ordinal}"),
                &uses,
                origins.complete,
            )?;
        }
        bind(origins, &at, &uses, uses.is_empty() || origins.complete)?;
        all.extend(uses);
    }
    bind(origins, "columns", &all, all.is_empty() || origins.complete)
}

fn navigation(
    origins: &mut ForgeOrigins,
    page: &PageEvidence<'_, '_>,
) -> Result<(), EvidenceError> {
    let forge = page.compiled.dataforge();
    let mut sites = vec![site(
        &origins.scope,
        "dataforgeRecordTable",
        "definitionCount",
        Vec::new(),
        SourceUseRole::Selector,
    )?];
    if forge.record_count() != 0 {
        for label in ["fileNameOffsets", "recordNameOffsets", "structIndices"] {
            sites.push(site(
                &origins.scope,
                "dataforgeRecordSurvey",
                label,
                vec![0, forge.record_count() as u64],
                SourceUseRole::Selector,
            )?);
        }
    }
    sites.extend(uses(
        origins,
        page.survey.selected(page.mode),
        SourceUseRole::Selector,
    )?);
    if page.survey.selected_by_guid() {
        sites.push(site(
            &origins.scope,
            "dataforgeRecordSurvey",
            "guidWords",
            vec![0, forge.record_count() as u64],
            SourceUseRole::Selector,
        )?);
    }
    let reference_complete =
        !page.compiled.has_reference_filter() || page.survey.references_complete();
    let complete = origins.complete
        && page.survey.complete(page.mode)
        && reference_complete
        && forge.path_order().len() == forge.record_count();
    bind(origins, "total", &sites, complete)?;
    if page.cursor {
        let examined: Vec<_> = page
            .examined
            .iter()
            .map(|row| u64::from(row.record().index()))
            .collect();
        if !examined.is_empty() {
            sites.push(site(
                &origins.scope,
                "dataforgeRecordSurvey",
                "decodedRecordSlots",
                examined,
                SourceUseRole::Selector,
            )?);
        }
        bind(origins, "next_cursor", &sites, complete)?;
    }
    Ok(())
}

fn rows(origins: &mut ForgeOrigins, page: &PageEvidence<'_, '_>) -> Result<(), EvidenceError> {
    let mut row_count = 0;
    let mut failed_count = 0;
    let mut output = Vec::new();
    let mut output_complete = true;
    let mut failures = Vec::new();
    let mut unresolved = Vec::new();
    for row in page.rows {
        let record = row.record().index();
        let path = record_site(origins, record, "fileName", SourceUseRole::Conversion)?;
        if let Ok(values) = row.cells() {
            let at = format!("rows/{row_count}");
            row_count += 1;
            bind(
                origins,
                &field_path(&at, RECORD_PATH_COLUMN)?,
                slice::from_ref(&path),
                origins.complete,
            )?;
            let mut membership = uses(
                origins,
                page.survey.reference_for(record),
                SourceUseRole::Selector,
            )?;
            let complete = origins.complete
                && (!page.compiled.has_reference_filter() || !membership.is_empty());
            membership.push(path);
            bind(origins, &at, &membership, complete)?;
            output.extend(membership);
            output_complete &= complete;
            row_cells(origins, page, row, &at, values)?;
            unresolved.extend(follow_inputs(origins, page.compiled, row)?);
        } else {
            let at = format!("failed/{failed_count}");
            failed_count += 1;
            failed_row(origins, &at, record, &path)?;
            failures.push(path);
        }
    }
    bind(origins, "rows", &output, output_complete)?;
    if !failures.is_empty() {
        bind(origins, "failed", &failures, origins.complete)?;
    }
    bind(
        origins,
        "unresolved_references",
        &unresolved,
        unresolved.is_empty() || origins.complete,
    )
}

fn row_cells(
    origins: &mut ForgeOrigins,
    page: &PageEvidence<'_, '_>,
    row: &Row<'_>,
    at: &str,
    values: &[Option<Value<'_>>],
) -> Result<(), EvidenceError> {
    let original = row.cell_sources().unwrap_or_default();
    for &column in page.columns {
        let Some(name) = page.compiled.columns().get(column) else {
            continue;
        };
        let field = field_path(at, name)?;
        let Some(cell) = original.get(column) else {
            bind_missing(origins, &field, row.record().index())?;
            continue;
        };
        let mut sources = uses(origins, cell.traversed(), SourceUseRole::Payload)?;
        if sources.is_empty() {
            sources.push(record_site(
                origins,
                row.record().index(),
                "sourceType",
                SourceUseRole::Selector,
            )?);
        }
        bind(origins, &field, &sources, origins.complete)?;
        match values.get(column).and_then(Option::as_ref) {
            Some(Value::Array(items)) => {
                for (ordinal, (route, item)) in cell.values().iter().zip(items.iter()).enumerate() {
                    let uses = uses(origins, route.iter().copied(), SourceUseRole::Payload)?;
                    let at = format!("{field}/{ordinal}");
                    bind(origins, &at, &uses, origins.complete)?;
                    error_fields(origins, &at, item, &uses)?;
                }
            }
            Some(value) => error_fields(origins, &field, value, &sources)?,
            None => {}
        }
    }
    Ok(())
}

fn error_fields(
    origins: &mut ForgeOrigins,
    path: &str,
    value: &Value<'_>,
    sources: &[SourceUse],
) -> Result<(), EvidenceError> {
    if let Value::Error(error) = value {
        bind(origins, &format!("{path}/error"), sources, origins.complete)?;
        if error.dangling_guid().is_some() {
            bind(origins, &format!("{path}/guid"), sources, origins.complete)?;
        }
    }
    Ok(())
}

fn bind_missing(origins: &mut ForgeOrigins, path: &str, record: u32) -> Result<(), EvidenceError> {
    let layout = record_site(origins, record, "sourceType", SourceUseRole::Selector)?;
    bind(origins, path, &[layout], origins.complete)
}

fn failed_row(
    origins: &mut ForgeOrigins,
    at: &str,
    record: u32,
    path: &SourceUse,
) -> Result<(), EvidenceError> {
    let name = record_site(origins, record, "recordName", SourceUseRole::Conversion)?;
    let layout = record_site(origins, record, "sourceType", SourceUseRole::Selector)?;
    bind(
        origins,
        &format!("{at}/record_path"),
        slice::from_ref(path),
        origins.complete,
    )?;
    bind(
        origins,
        &format!("{at}/name"),
        slice::from_ref(&name),
        origins.complete,
    )?;
    bind(
        origins,
        &format!("{at}/error"),
        slice::from_ref(&layout),
        false,
    )?;
    bind(origins, at, &[path.clone(), name, layout], false)
}

fn field_path(parent: &str, name: &str) -> Result<String, EvidenceError> {
    let mut path = FieldPath::root();
    path.push_field(name)?;
    Ok(format!("{parent}{}", path.as_str()))
}

fn bind(
    origins: &mut ForgeOrigins,
    path: &str,
    uses: &[SourceUse],
    complete: bool,
) -> Result<(), EvidenceError> {
    origins.put(path, origins.uses.origin(uses, complete))
}

fn follow_inputs(
    origins: &ForgeOrigins,
    query: &CompiledQuery<'_>,
    row: &Row<'_>,
) -> Result<Vec<SourceUse>, EvidenceError> {
    let mut result = Vec::new();
    let cells = row.cell_sources().unwrap_or_default();
    for (kind, cell) in query.column_kinds().iter().zip(cells) {
        if !kind.follows_references() {
            continue;
        }
        let mut used = uses(origins, cell.follow_inputs(), SourceUseRole::Selector)?;
        if used.is_empty() {
            used.push(record_site(
                origins,
                row.record().index(),
                "sourceType",
                SourceUseRole::Selector,
            )?);
        }
        result.append(&mut used);
    }
    Ok(result)
}

fn ordinal_ranges(indices: impl IntoIterator<Item = u32>) -> Vec<u64> {
    let mut ranges = Vec::new();
    for index in indices.into_iter().map(u64::from) {
        if let Some(end) = ranges.last_mut().filter(|end| **end == index) {
            *end += 1;
        } else {
            ranges.extend([index, index + 1]);
        }
    }
    ranges
}
