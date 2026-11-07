// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::time::SystemTime;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, ExportDiagnosticSeverity, ExportHealth,
    ExportStageStatus, Kind,
};

use super::error::{Error, Kind as ErrorKind};
use super::inventory::{Disposition, Inventory};

pub(crate) const fn options(generated_at: SystemTime) -> CatalogOptions<'static> {
    CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(generated_at)
    }
}

pub(crate) fn kinds(inventory: &Inventory) -> super::Result<Vec<Kind>> {
    let names: BTreeSet<_> = inventory
        .fields
        .iter()
        .filter_map(|mapping| match &mapping.disposition {
            Disposition::Exported { group, .. } => Some(group.as_str()),
            _ => None,
        })
        .collect();
    from_names(names)
}

pub(crate) fn from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> super::Result<Vec<Kind>> {
    let mut selected = Vec::new();
    for name in names {
        let kind = Kind::ALL
            .into_iter()
            .find(|kind| kind.name() == name)
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Mapping,
                    format!("unknown mapped catalog kind {name}"),
                )
            })?;
        if !selected.contains(&kind) {
            selected.push(kind);
        }
    }
    Ok(selected)
}

fn verify_kind(health: &ExportHealth, kind: Kind) -> super::Result<()> {
    let actual = health.kind(kind).ok_or_else(|| {
        Error::new(
            ErrorKind::Input,
            format!("mapped kind {kind} has no production health"),
        )
    })?;
    if actual.read != ExportStageStatus::Complete
        || actual.projection != ExportStageStatus::Complete
        || !actual.source_inventory_complete
        || !actual.field_inventory_complete
        || !actual.failed_records.is_empty()
        || !actual.failed_entries.is_empty()
        || health
            .failed_records
            .iter()
            .any(|source| source.kind == Some(kind))
        || health.failed_entries.iter().any(|entry| entry.kind == kind)
        || health.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == Some(kind)
                && diagnostic.severity != ExportDiagnosticSeverity::Information
        })
    {
        return Err(Error::new(ErrorKind::Mapping,
            format!("mapped kind {kind} has incomplete source/projection health; failed physical Records={:?}; failed Entries={:?}",
                actual.failed_records.iter().map(|source| source.record_index).collect::<Vec<_>>(),
                actual.failed_entries.iter().map(|entry| (entry.entry_index, entry.operation)).collect::<Vec<_>>(),
            )).into());
    }
    Ok(())
}

pub(crate) fn verify(health: &ExportHealth, kinds: &[Kind]) -> super::Result<()> {
    if health.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind.is_none() && diagnostic.severity != ExportDiagnosticSeverity::Information
    }) || health
        .failed_records
        .iter()
        .any(|source| source.kind.is_none())
    {
        return Err(Error::new(
            ErrorKind::Mapping,
            "unscoped source health cannot establish independence from mapped fields",
        )
        .into());
    }
    if !health.is_complete()
        && !health.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind.is_some()
                && diagnostic.severity != ExportDiagnosticSeverity::Information
        })
    {
        return Err(Error::new(
            ErrorKind::Mapping,
            "incomplete audit health has no established source scope",
        )
        .into());
    }
    for &kind in kinds {
        verify_kind(health, kind)?;
    }
    Ok(())
}
