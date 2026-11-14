// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{
    Archive, CancelToken, DataForge, Filter, Guid, SourceValueState, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::planet_palette_reference::PlanetPaletteReference;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_archive_path::{self, LooseArchivePathState};
use crate::catalog_read::loose_json::{LooseJson, LooseJsonFailure};
use crate::catalog_read::original::{EntryCapture, EntryReceipt, PhysicalEntryIdentity};
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
use crate::starmap_index;
use crate::{CatalogError, CatalogOptions, Kind};

const FOLDER: &str = "data/libs/planets/resources/";
const EXTENSION: &str = "resource";
const KIND: Kind = Kind::PlanetResources;
const SOURCE_TYPE: &str = "PlanetResource";

pub(crate) type PlanetResourceTable = CatalogTable<PlanetResource>;

/// One physical loose planet resource and its original definition fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PlanetResource {
    identity: PhysicalEntryIdentity,
    guid: SourcedValue<CatalogGuid>,
    guid_raw: SourcedValue<String>,
    #[schemars(extend("x-gameText" = true))]
    name: SourcedValue<String>,
    minable: SourcedValue<bool>,
    tint_palette_path: SourcedValue<String>,
    tint_palette: PlanetPaletteReference,
    issues: Vec<PlanetResourceIssue>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct PlanetResourceIssue {
    field: &'static str,
    code: ExportDiagnosticCode,
}

struct Definitions<'input, 'forge, 'source> {
    archive: &'input Archive,
    forge: &'input DataForge<'source>,
    original: &'input OriginalCatalogInput<'forge, 'source>,
    capture: &'input EntryCapture<'input>,
    filter: Option<&'input Filter>,
    cancel: Option<&'input CancelToken>,
    keys: Option<p4kx_extraction::PathKeys>,
}

pub(crate) fn read(
    archive: &Archive,
    forge: &DataForge<'_>,
    original: &OriginalCatalogInput<'_, '_>,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
) -> Result<PlanetResourceTable, CatalogError> {
    let mut reader = Definitions {
        archive,
        forge,
        original,
        capture,
        filter: options.filter,
        cancel: options.cancel,
        keys: None,
    };
    let selected = archive
        .entries()
        .iter()
        .filter(|entry| in_scope(entry.path()));
    let mut available = false;
    let mut rows = Vec::new();
    ledger.inventory(KIND);
    for entry in selected {
        parsed_entry_json::check_cancel(options.cancel)?;
        available = true;
        if options
            .filter
            .is_some_and(|filter| !filter.matches(entry.path()))
        {
            continue;
        }
        let source =
            EntrySource::outer(capture, archive, entry)?.operation(EntryOperation::SourceJson);
        let projected = match LooseJson::read(archive, entry, &source, options.cancel)? {
            Ok(read) => reader.project(&read, source.receipt())?,
            Err(error) => Err(error),
        };
        match projected {
            Ok(row) => {
                source.record(ledger, KIND, SourceDisposition::Projected);
                parsed_entry_json::push(&mut rows, row)?;
            }
            Err(error) => {
                source.diagnosed(
                    ledger,
                    KIND,
                    error.disposition,
                    error.diagnosis(SOURCE_TYPE),
                );
            }
        }
    }
    annotate_duplicates(&mut rows, ledger)?;
    let availability = if available {
        DatasetAvailability::Available
    } else if ledger.archive_inventory_problem.is_some() {
        DatasetAvailability::Unavailable
    } else {
        DatasetAvailability::Absent
    };
    Ok(CatalogTable { availability, rows })
}

impl Definitions<'_, '_, '_> {
    fn project(
        &mut self,
        read: &LooseJson,
        receipt: &EntryReceipt,
    ) -> Result<Result<PlanetResource, LooseJsonFailure>, CatalogError> {
        let parsed = match read.parse(self.capture, self.cancel)? {
            Ok(parsed) => parsed,
            Err(error) => return Ok(Err(error)),
        };
        let Some(object) = EntryJsonObject::new(&parsed, parsed.root, self.capture.source_uses())?
        else {
            return Ok(Err(LooseJsonFailure::malformed()));
        };
        let guid = object.field("guid", entry_json_object::guid)?;
        let guid_raw = object.field("guid", entry_json_object::text)?;
        let name = object.field("name", entry_json_object::text)?;
        let minable = object.field("bMinable", entry_json_object::boolean)?;
        let tint_palette_path = object.field("tintPalettePath", entry_json_object::text)?;
        let tint_palette = self.resolve(&tint_palette_path)?;
        let issues = field_issues(&guid, &name, &minable, tint_palette.state());
        Ok(Ok(PlanetResource {
            identity: receipt.physical_payload_file(starmap_index::key)?,
            guid,
            guid_raw,
            name,
            minable,
            tint_palette_path,
            tint_palette,
            issues,
        }))
    }

    fn resolve(
        &mut self,
        path: &SourcedValue<String>,
    ) -> Result<PlanetPaletteReference, CatalogError> {
        if let Some(link) = PlanetPaletteReference::of_path(
            path,
            self.forge,
            self.original,
            self.capture,
            self.filter,
            self.cancel,
        )? {
            return Ok(link);
        }
        if self.keys.is_none() {
            self.keys = Some(loose_archive_path::keys(self.archive, self.cancel)?);
        }
        let keys = self
            .keys
            .as_ref()
            .ok_or(crate::ExportAccountingError::ConflictingSourceDatum)?;
        let resolution = loose_archive_path::resolve(
            &keys.index(self.archive),
            self.capture,
            self.cancel,
            path,
            |_| true,
        )?;
        Ok(PlanetPaletteReference::ArchiveEntry { resolution })
    }
}

fn field_issues(
    guid: &SourcedValue<CatalogGuid>,
    name: &SourcedValue<String>,
    minable: &SourcedValue<bool>,
    palette: Option<LooseArchivePathState>,
) -> Vec<PlanetResourceIssue> {
    let mut issues = Vec::new();
    if matches!(
        guid.state(),
        SourceValueState::Absent | SourceValueState::Invalid | SourceValueState::Null
    ) {
        issues.push(PlanetResourceIssue {
            field: "guid",
            code: ExportDiagnosticCode::MalformedSource,
        });
    } else if guid.value().is_some_and(|guid| guid.0 == Guid::default()) {
        issues.push(PlanetResourceIssue {
            field: "guid",
            code: ExportDiagnosticCode::EmptyValue,
        });
    }
    for (field, state) in [("name", name.state()), ("bMinable", minable.state())] {
        if state == SourceValueState::Invalid {
            issues.push(PlanetResourceIssue {
                field,
                code: ExportDiagnosticCode::MalformedSource,
            });
        }
    }
    let code = match palette {
        Some(LooseArchivePathState::Invalid | LooseArchivePathState::WrongFamily) => {
            Some(ExportDiagnosticCode::MalformedSource)
        }
        Some(LooseArchivePathState::Missing) => Some(ExportDiagnosticCode::DanglingReference),
        Some(LooseArchivePathState::Unreadable) => Some(ExportDiagnosticCode::UnreadableSource),
        Some(LooseArchivePathState::Unavailable | LooseArchivePathState::Filtered) => {
            Some(ExportDiagnosticCode::CatalogTargetUnavailable)
        }
        Some(LooseArchivePathState::Ambiguous) => Some(ExportDiagnosticCode::AmbiguousIdentity),
        _ => None,
    };
    if let Some(code) = code {
        issues.push(PlanetResourceIssue {
            field: "tintPalettePath",
            code,
        });
    }
    issues
}

fn annotate_duplicates(
    rows: &mut [PlanetResource],
    ledger: &mut ExportLedger,
) -> Result<(), CatalogError> {
    let mut counts = HashMap::<CatalogGuid, usize>::new();
    counts
        .try_reserve(rows.len())
        .map_err(CatalogError::SourceUseAllocation)?;
    for row in rows.iter() {
        if let Some(guid) = row.guid.value().filter(|guid| guid.0 != Guid::default()) {
            *counts.entry(*guid).or_default() += 1;
        }
    }
    for row in rows {
        if row
            .guid
            .value()
            .and_then(|guid| counts.get(guid))
            .is_some_and(|count| *count > 1)
        {
            row.issues.push(PlanetResourceIssue {
                field: "guid",
                code: ExportDiagnosticCode::AmbiguousIdentity,
            });
        }
        for issue in &row.issues {
            ledger.field_diagnostic(DiagnosticObservation {
                kind: Some(KIND),
                stage: ExportDiagnosticStage::SourceField,
                code: issue.code,
                severity: ExportDiagnosticSeverity::Information,
                source_role: Some(ExportSourceRole::DerivedInput),
                warning_code: None,
                count: 1,
                count_unit: ExportDiagnosticUnit::Values,
                examples: Vec::new(),
            });
        }
    }
    Ok(())
}

fn in_scope(path: &str) -> bool {
    starts_with_folder(path, FOLDER)
        && path
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case(EXTENSION))
}

#[cfg(test)]
#[path = "planet_resource_tests.rs"]
mod tests;
