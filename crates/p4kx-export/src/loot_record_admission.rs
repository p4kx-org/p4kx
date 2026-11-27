// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeHeaderField, FieldOrigin, SourceLocation, SourceUse,
    SourceUseRef, SourceUseRole, SourceValue, SourcedValue,
};

use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::loot::LootDocumentAdmission;
use crate::catalog_query_session::checkpoint;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::starmap_containers::PlacementScope;
use crate::{CatalogError, ExportAccountingError, Filter};

pub(crate) struct Admission {
    pub(crate) status: LootDocumentAdmission,
    pub(crate) record: Option<SourcedValue<u32>>,
    pub(crate) survey: Option<FieldOrigin>,
    pub(crate) failed_source_use: Option<SourceUseRef>,
}

pub(crate) struct RecordPaths {
    pub(crate) status: LootDocumentAdmission,
    pub(crate) record: Option<SourcedValue<u32>>,
    first_unreadable: Option<u32>,
}

pub(crate) fn scan(
    forge: &DataForge<'_>,
    origins: &CatalogOrigins,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<RecordPaths, CatalogError> {
    let mut first_unreadable = None;
    for record in forge.records() {
        checkpoint(cancel)?;
        match record.record_path() {
            Ok(path) if PlacementScope::admits_loot_path(path, filter) => {
                let mut uses = Vec::new();
                let original = origins.record(record.index());
                original.append_header_input(
                    RecordHeaderInput::NormalizedPath,
                    SourceUseRole::Selector,
                    &mut uses,
                )?;
                original.append_header_input(
                    RecordHeaderInput::Field(RecordHeaderField::RecordIndex),
                    SourceUseRole::Payload,
                    &mut uses,
                )?;
                return Ok(RecordPaths {
                    status: LootDocumentAdmission::Selected,
                    record: Some(origins.uses.value(
                        SourceValue::Present(record.index()),
                        &uses,
                        true,
                    )),
                    first_unreadable,
                });
            }
            Ok(_) => {}
            Err(_) => {
                first_unreadable.get_or_insert_with(|| record.index());
            }
        }
    }
    let status = if first_unreadable.is_some() {
        LootDocumentAdmission::Unavailable
    } else {
        LootDocumentAdmission::Absent
    };
    Ok(RecordPaths {
        status,
        record: None,
        first_unreadable,
    })
}

pub(crate) fn admit(
    forge: &DataForge<'_>,
    origins: &CatalogOrigins,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<Admission, CatalogError> {
    let paths = scan(forge, origins, filter, cancel)?;
    if paths.record.is_some() {
        return Ok(Admission {
            status: paths.status,
            record: paths.record,
            survey: None,
            failed_source_use: failed_filename(origins, paths.first_unreadable)?,
        });
    }
    exhausted_admission(forge, origins, paths.first_unreadable)
}

fn exhausted_admission(
    forge: &DataForge<'_>,
    origins: &CatalogOrigins,
    first_unreadable: Option<u32>,
) -> Result<Admission, CatalogError> {
    let mut uses = vec![record_count_source(origins)?];
    if forge.record_count() != 0 {
        let table = forge
            .record_table_span()
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        uses.push(observe_span(
            origins,
            "dataforgeRecordTable",
            RecordHeaderField::FileName.name(),
            table,
        )?);
        if let Some(texts) = forge.text_table_span() {
            uses.push(observe_span(
                origins,
                "dataforgeTextTable",
                RecordHeaderField::FileName.name(),
                texts,
            )?);
        }
    }
    let complete = first_unreadable.is_none();
    Ok(Admission {
        status: if complete {
            LootDocumentAdmission::Absent
        } else {
            LootDocumentAdmission::Unavailable
        },
        record: None,
        survey: Some(origins.uses.origin(&uses, complete)),
        failed_source_use: failed_filename(origins, first_unreadable)?,
    })
}

fn failed_filename(
    origins: &CatalogOrigins,
    first_unreadable: Option<u32>,
) -> Result<Option<SourceUseRef>, CatalogError> {
    first_unreadable
        .map(|record| {
            origins
                .record(record)
                .observe_header(RecordHeaderField::FileName, SourceUseRole::Attempted)
                .map(|used| used.reference().clone())
        })
        .transpose()
}

pub(super) fn record_count_source(origins: &CatalogOrigins) -> Result<SourceUse, CatalogError> {
    observe_span(
        origins,
        "dataforgeHeader",
        "recordCount",
        DataForgeHeaderField::RecordCount.byte_span(),
    )
}

fn observe_span(
    origins: &CatalogOrigins,
    namespace: &str,
    field: &str,
    span: Range<u64>,
) -> Result<SourceUse, CatalogError> {
    Ok(origins.dataforge.at(
        SourceLocation::Subresource {
            namespace: namespace.to_owned(),
            indices: vec![span.start, span.end],
            field: field.to_owned(),
        },
        SourceUseRole::Selector,
    )?)
}
