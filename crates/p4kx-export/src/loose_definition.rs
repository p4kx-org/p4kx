// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::slice;

use p4kx_extraction::{Archive, Entry, Guid, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_archive_path::LooseArchivePathState;
use crate::catalog_read::loose_json::{LooseJson, LooseJsonFailure};
use crate::catalog_read::original::{EntryCapture, PhysicalEntryIdentity};
use crate::catalog_read::parsed_entry_json;
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::entry_source::EntrySource;
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::guid_text::CatalogGuid;
use crate::health_diagnostic_observation::DiagnosticObservation;
use crate::schema_fields::every_field_required;
use crate::selection::starts_with_folder;
use crate::{CatalogError, CatalogOptions, Kind, starmap_index};

/// One canonical Archive JSON family and its physical Entry selector.
#[derive(Clone, Copy)]
pub(crate) struct DefinitionFamily {
    kind: Kind,
    folder: &'static str,
    extension: &'static str,
    source_type: &'static str,
    guid_field: &'static str,
}

impl DefinitionFamily {
    pub(crate) const fn new(
        kind: Kind,
        folder: &'static str,
        extension: &'static str,
        source_type: &'static str,
        guid_field: &'static str,
    ) -> Self {
        Self {
            kind,
            folder,
            extension,
            source_type,
            guid_field,
        }
    }

    pub(crate) fn includes(self, path: &str) -> bool {
        starts_with_folder(path, self.folder)
            && path
                .rsplit_once('.')
                .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case(self.extension))
    }
}

/// A represented issue of one authored field in a readable loose definition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct DefinitionIssue {
    pub(crate) field: &'static str,
    pub(crate) code: ExportDiagnosticCode,
}

impl DefinitionIssue {
    /// Returns the authored field name.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        self.field
    }

    /// Returns the represented source issue.
    #[must_use]
    pub const fn code(&self) -> ExportDiagnosticCode {
        self.code
    }

    pub(crate) fn record(&self, ledger: &mut ExportLedger, kind: Kind) {
        ledger.field_diagnostic(DiagnosticObservation {
            kind: Some(kind),
            stage: ExportDiagnosticStage::SourceField,
            code: self.code,
            severity: ExportDiagnosticSeverity::Information,
            source_role: Some(ExportSourceRole::DerivedInput),
            warning_code: None,
            count: 1,
            count_unit: ExportDiagnosticUnit::Values,
            examples: Vec::new(),
        });
    }
}

/// Selects physical Entries and optionally their authored GUID.
#[derive(Clone, Copy)]
pub(crate) enum Selection<'path> {
    All,
    EntryIndex(u32),
    Path(&'path str),
    Guid(Guid),
}

impl Selection<'_> {
    pub(crate) fn entries(self, archive: &Archive) -> &[Entry] {
        match self {
            Self::EntryIndex(index) => usize::try_from(index)
                .ok()
                .and_then(|index| archive.entries().get(index))
                .map_or(&[][..], slice::from_ref),
            _ => archive.entries(),
        }
    }

    const fn guid(self) -> Option<Guid> {
        if let Self::Guid(guid) = self {
            Some(guid)
        } else {
            None
        }
    }
}

pub(crate) trait LooseDefinition {
    fn guid(&self) -> &SourcedValue<CatalogGuid>;
    fn issues(&mut self) -> &mut Vec<DefinitionIssue>;
}

pub(crate) fn read<T: LooseDefinition>(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
    family: DefinitionFamily,
    project: impl FnMut(PhysicalEntryIdentity, &EntryJsonObject<'_, '_>) -> Result<T, CatalogError>,
) -> Result<CatalogTable<T>, CatalogError> {
    read_selected(
        archive,
        capture,
        ledger,
        options,
        family,
        Selection::All,
        project,
    )
}

pub(crate) fn read_selected<T: LooseDefinition>(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
    family: DefinitionFamily,
    selection: Selection<'_>,
    mut project: impl FnMut(PhysicalEntryIdentity, &EntryJsonObject<'_, '_>) -> Result<T, CatalogError>,
) -> Result<CatalogTable<T>, CatalogError> {
    let path = match selection {
        Selection::Path(path) => Some(starmap_index::key(path)),
        _ => None,
    };
    let mut available = false;
    let mut rows = Vec::new();
    ledger.inventory(family.kind);
    for entry in selection
        .entries(archive)
        .iter()
        .filter(|entry| family.includes(entry.path()))
    {
        parsed_entry_json::check_cancel(options.cancel)?;
        if path
            .as_ref()
            .is_some_and(|path| *path != starmap_index::key(entry.path()))
        {
            continue;
        }
        available = true;
        if options
            .filter
            .is_some_and(|filter| !filter.matches(entry.path()))
        {
            continue;
        }
        let source =
            EntrySource::outer(capture, archive, entry)?.operation(EntryOperation::SourceJson);
        let outcome = match LooseJson::read(archive, entry, &source, options.cancel)? {
            Ok(read) => project_entry(
                &read,
                &source,
                capture,
                options.cancel,
                selection.guid(),
                family.guid_field,
                &mut project,
            )?,
            Err(error) => Err(error),
        };
        retain_outcome(&mut rows, ledger, source, family, selection, outcome)?;
    }
    diagnostics(&mut rows, ledger, family)?;
    let availability = if available {
        DatasetAvailability::Available
    } else if ledger.archive_inventory_problem.is_some() {
        DatasetAvailability::Unavailable
    } else {
        DatasetAvailability::Absent
    };
    Ok(CatalogTable { availability, rows })
}

fn retain_outcome<T>(
    rows: &mut Vec<T>,
    ledger: &mut ExportLedger,
    source: EntrySource,
    family: DefinitionFamily,
    selection: Selection<'_>,
    outcome: Result<Option<T>, LooseJsonFailure>,
) -> Result<(), CatalogError> {
    match outcome {
        Ok(Some(row)) => {
            source.record(ledger, family.kind, SourceDisposition::Projected);
            parsed_entry_json::push(rows, row)?;
        }
        Ok(None) => source.selection_candidate().record(
            ledger,
            family.kind,
            SourceDisposition::SkippedAfterRead,
        ),
        Err(error) => {
            let source = if selection.guid().is_some() {
                source.selection_candidate()
            } else {
                source
            };
            source.diagnosed(
                ledger,
                family.kind,
                error.disposition,
                error.diagnosis(family.source_type),
            );
        }
    }
    Ok(())
}

fn project_entry<T>(
    read: &LooseJson,
    source: &EntrySource,
    capture: &EntryCapture<'_>,
    cancel: Option<&p4kx_extraction::CancelToken>,
    wanted: Option<Guid>,
    guid_field: &str,
    project: &mut impl FnMut(PhysicalEntryIdentity, &EntryJsonObject<'_, '_>) -> Result<T, CatalogError>,
) -> Result<Result<Option<T>, LooseJsonFailure>, CatalogError> {
    let parsed = match read.parse(capture, cancel)? {
        Ok(parsed) => parsed,
        Err(error) => return Ok(Err(error)),
    };
    let Some(object) = EntryJsonObject::new(&parsed, parsed.root, capture.source_uses())? else {
        return Ok(Err(LooseJsonFailure::malformed()));
    };
    if let Some(wanted) = wanted {
        let guid = object.field(guid_field, entry_json_object::guid)?;
        match guid.state() {
            SourceValueState::Absent | SourceValueState::Null => return Ok(Ok(None)),
            SourceValueState::Present => {
                if wanted == Guid::default() || guid.value() != Some(&CatalogGuid(wanted)) {
                    return Ok(Ok(None));
                }
            }
            _ => return Ok(Err(LooseJsonFailure::malformed())),
        }
    }
    let identity = source.receipt().physical_payload_file(starmap_index::key)?;
    project(identity, &object).map(|row| Ok(Some(row)))
}

pub(crate) fn identity_issue(
    field: &'static str,
    guid: &SourcedValue<CatalogGuid>,
) -> Option<DefinitionIssue> {
    let code = match (guid.state(), guid.value()) {
        (SourceValueState::Absent | SourceValueState::Invalid | SourceValueState::Null, _) => {
            ExportDiagnosticCode::MalformedSource
        }
        (_, Some(guid)) if guid.0 == Guid::default() => ExportDiagnosticCode::EmptyValue,
        _ => return None,
    };
    Some(DefinitionIssue { field, code })
}

pub(crate) const fn reference_issue(state: LooseArchivePathState) -> Option<ExportDiagnosticCode> {
    match state {
        LooseArchivePathState::Invalid | LooseArchivePathState::WrongFamily => {
            Some(ExportDiagnosticCode::MalformedSource)
        }
        LooseArchivePathState::Missing => Some(ExportDiagnosticCode::DanglingReference),
        LooseArchivePathState::Unreadable => Some(ExportDiagnosticCode::UnreadableSource),
        LooseArchivePathState::Unavailable | LooseArchivePathState::Filtered => {
            Some(ExportDiagnosticCode::CatalogTargetUnavailable)
        }
        LooseArchivePathState::Ambiguous => Some(ExportDiagnosticCode::AmbiguousIdentity),
        _ => None,
    }
}

fn diagnostics<T: LooseDefinition>(
    rows: &mut [T],
    ledger: &mut ExportLedger,
    family: DefinitionFamily,
) -> Result<(), CatalogError> {
    let mut counts = HashMap::<CatalogGuid, usize>::new();
    counts
        .try_reserve(rows.len())
        .map_err(CatalogError::SourceUseAllocation)?;
    for row in rows.iter() {
        if let Some(guid) = row.guid().value().filter(|guid| guid.0 != Guid::default()) {
            *counts.entry(*guid).or_default() += 1;
        }
    }
    for row in rows {
        if row
            .guid()
            .value()
            .and_then(|guid| counts.get(guid))
            .is_some_and(|count| *count > 1)
        {
            row.issues().push(DefinitionIssue {
                field: family.guid_field,
                code: ExportDiagnosticCode::AmbiguousIdentity,
            });
        }
        for issue in row.issues() {
            issue.record(ledger, family.kind);
        }
    }
    Ok(())
}
