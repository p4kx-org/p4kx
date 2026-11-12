// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::BTreeSet;

use p4kx_extraction::{
    DataForgeOrigin, FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath, FieldPathError,
    InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses,
};
use thiserror::Error;

use crate::forge_shapes::RecordSearchOrigins;
use crate::record_search::Row;

#[derive(Debug, Error)]
pub(crate) enum EvidenceError {
    #[error(transparent)]
    Source(#[from] SourceEvidenceError),
    #[error(transparent)]
    Path(#[from] FieldPathError),
    #[error(transparent)]
    Duplicate(#[from] FieldOriginsError),
}

#[derive(Clone, Copy)]
pub(crate) enum Navigation {
    Complete,
    HeaderWitness,
}

pub(crate) struct Survey {
    record_count: usize,
    readable_paths: Vec<u64>,
    typed_records: Vec<u64>,
    tested_definitions: Vec<u64>,
    complete: bool,
}

impl Survey {
    pub(crate) fn of(
        record_count: usize,
        readable_paths: &[u32],
        typed_records: Vec<u32>,
        tested_definitions: BTreeSet<u32>,
        complete: bool,
    ) -> Self {
        Self {
            record_count,
            readable_paths: record_ranges(Cow::Borrowed(readable_paths), record_count),
            typed_records: record_ranges(Cow::Owned(typed_records), record_count),
            tested_definitions: consecutive_ranges(tested_definitions),
            complete,
        }
    }

    fn write(
        &self,
        origins: &mut Origins,
        cursor: bool,
        navigation: Navigation,
    ) -> Result<(), EvidenceError> {
        let mut sources = vec![site(
            &origins.scope,
            "dataforgeRecordTable",
            "definitionCount",
            Vec::new(),
            SourceUseRole::Selector,
        )?];
        let complete =
            origins.complete && self.complete && matches!(navigation, Navigation::Complete);
        if matches!(navigation, Navigation::Complete) {
            if self.record_count != 0 {
                sources.push(site(
                    &origins.scope,
                    "dataforgeRecordSurvey",
                    "fileNameOffsets",
                    vec![0, self.record_count as u64],
                    SourceUseRole::Selector,
                )?);
            }
            for (field, indices) in [
                ("readableFileNames", &self.readable_paths),
                ("testedRecordTypes", &self.typed_records),
                ("testedTypeNames", &self.tested_definitions),
            ] {
                if !indices.is_empty() {
                    sources.push(site(
                        &origins.scope,
                        "dataforgeRecordSurvey",
                        field,
                        indices.clone(),
                        SourceUseRole::Selector,
                    )?);
                }
            }
        }
        origins.put("total", origins.uses.origin(&sources, complete))?;
        if cursor {
            if matches!(navigation, Navigation::Complete) && !self.readable_paths.is_empty() {
                sources.push(site(
                    &origins.scope,
                    "dataforgeRecordSurvey",
                    "orderNameOffsets",
                    self.readable_paths.clone(),
                    SourceUseRole::Selector,
                )?);
            }
            origins.put("next_cursor", origins.uses.origin(&sources, complete))?;
        }
        Ok(())
    }
}

pub(crate) fn project(
    origin: &DataForgeOrigin,
    survey: &Survey,
    rows: &[Row],
    cursor: bool,
    navigation: Navigation,
) -> Result<RecordSearchOrigins, EvidenceError> {
    let (source, complete) = match origin {
        DataForgeOrigin::Archive(entry) => (
            SourceInput::Archive(entry),
            entry.ancestors().all(|step| step.is_some()),
        ),
        DataForgeOrigin::File(file) => (SourceInput::File(file), true),
        _ => (SourceInput::Unknown, false),
    };
    let uses = SourceUses::with_compact_use_refs();
    let scope = uses.observe(source, InputScope::Payload)?;
    let mut origins = Origins {
        uses,
        scope,
        fields: FieldOrigins::new(),
        complete,
    };
    origins.put("archive", origins.uses.origin(&[], true))?;
    survey.write(&mut origins, cursor, navigation)?;
    let mut contributors = Vec::new();
    for (position, row) in rows.iter().enumerate() {
        origins.row(row, position, &mut contributors)?;
    }
    origins.put(
        "records",
        origins
            .uses
            .origin(&contributors, rows.is_empty() || complete),
    )?;
    Ok(RecordSearchOrigins {
        fields: origins.fields,
        provenance: origins.uses.freeze()?,
    })
}

struct Origins {
    uses: SourceUses,
    scope: SourceScope,
    fields: FieldOrigins,
    complete: bool,
}

impl Origins {
    fn row(
        &mut self,
        row: &Row,
        position: usize,
        page: &mut Vec<SourceUse>,
    ) -> Result<(), EvidenceError> {
        for (output, original, role, definition) in [
            ("guid", "guid", SourceUseRole::Payload, None),
            ("name", "recordName", SourceUseRole::Conversion, None),
            ("path", "fileName", SourceUseRole::Conversion, None),
            (
                "type",
                "sourceType",
                SourceUseRole::Selector,
                Some(row.structure),
            ),
        ] {
            let field = self.scope.at(
                SourceLocation::DataForge {
                    record_index: row.record,
                    field: original.into(),
                },
                role,
            )?;
            let mut uses = vec![field];
            if let Some(definition) = definition {
                uses.push(site(
                    &self.scope,
                    "dataforgeStructDefinition",
                    "name",
                    vec![u64::from(definition)],
                    SourceUseRole::Payload,
                )?);
            }
            self.put(
                &format!("records/{position}/{output}"),
                self.uses.origin(&uses, self.complete),
            )?;
            page.extend(uses);
        }
        Ok(())
    }

    fn put(&mut self, pointer: &str, origin: FieldOrigin) -> Result<(), EvidenceError> {
        self.fields.insert(
            FieldPath::parse(&format!("/structuredContent/{pointer}"))?,
            origin,
        )?;
        Ok(())
    }
}

fn site(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    indices: Vec<u64>,
    role: SourceUseRole,
) -> Result<SourceUse, SourceEvidenceError> {
    scope.at(
        SourceLocation::Subresource {
            namespace: namespace.into(),
            indices,
            field: field.into(),
        },
        role,
    )
}

fn record_ranges(positions: Cow<'_, [u32]>, record_count: usize) -> Vec<u64> {
    if positions.is_empty() {
        return Vec::new();
    }
    if positions.len() == record_count {
        return vec![0, record_count as u64];
    }
    let mut positions = positions.into_owned();
    positions.sort_unstable();
    consecutive_ranges(positions)
}

fn consecutive_ranges(positions: impl IntoIterator<Item = u32>) -> Vec<u64> {
    let mut ranges = Vec::new();
    for position in positions.into_iter().map(u64::from) {
        if let Some(end) = ranges.last_mut().filter(|end| **end == position) {
            *end += 1;
        } else {
            ranges.extend([position, position + 1]);
        }
    }
    ranges
}
