// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Arc, OnceLock};

use p4kx_extraction::{
    Archive, ArchiveWatch, Build, CancelToken, ProvenanceMode, SourceInventory, SourceUse,
    SourceValue, SourcedValue,
};
use serde::Serialize;

use super::{GridRead, LocationPhysicsGrid};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::{
    EntryCapture, EntryRead, EntryXml, PhysicalEntryRef, with_entry_capture_mode,
};
use crate::entry_source::EntrySource;
use crate::export_ledger::ExportLedger;
use crate::starmap_parse::entxml::EntXmlFiles;
use crate::{CatalogError, ExportAccountingError};

/// Selects a canonical descriptor by both original directory ordinals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocationPhysicsDescriptorSelection {
    container: u32,
    descriptor: u32,
}

impl LocationPhysicsDescriptorSelection {
    /// Selects the outer container Entry and its specific inner descriptor Entry.
    #[must_use]
    pub const fn new(container: u32, descriptor: u32) -> Self {
        Self {
            container,
            descriptor,
        }
    }
}

/// Outcome of one direct descriptor lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum LocationPhysicsDescriptorStatus {
    /// The complete root directory has no selected container.
    NotFound,
    /// The canonical descriptor cannot be selected or decoded.
    Unavailable,
    /// The admitted canonical descriptor has another inner Entry ordinal.
    SelectorMismatch,
    /// The requested physical descriptor was read successfully.
    Read,
}

/// The direct read of one authored descriptor's location physics.
#[derive(Debug)]
#[non_exhaustive]
pub struct LocationPhysicsDescriptorSnapshot {
    /// The Build of the watched Archive.
    pub build: Option<Build>,
    /// Result of the requested descriptor admission.
    pub status: LocationPhysicsDescriptorStatus,
    /// Exact original descriptor used for this response, when decoded.
    pub descriptor: Option<SourcedValue<PhysicalEntryRef>>,
    /// Whether original grid declarations were observed in the selected descriptor.
    pub present: SourcedValue<bool>,
    /// The canonical descriptor row if it contains authored location physics.
    pub grid: Option<Arc<LocationPhysicsGrid>>,
    /// Exact original inputs and use sites of the response.
    pub source_inventory: SourceInventory,
}

/// Reads one physical location grid descriptor without walking system placements.
///
/// # Errors
/// Returns an error for cancellation, changed inputs or contradictory original source evidence.
pub fn read_location_physics_descriptor(
    archive: &Archive,
    selection: LocationPhysicsDescriptorSelection,
    cancel: Option<&CancelToken>,
) -> Result<LocationPhysicsDescriptorSnapshot, CatalogError> {
    read_location_physics_descriptor_with_provenance_mode(
        archive,
        selection,
        ProvenanceMode::MarkersOnly,
        cancel,
    )
}

/// Reads a location physics descriptor with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_location_physics_descriptor`.
pub fn read_location_physics_descriptor_with_provenance_mode(
    archive: &Archive,
    selection: LocationPhysicsDescriptorSelection,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<LocationPhysicsDescriptorSnapshot, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    with_entry_capture_mode(archive, provenance_mode, |capture| {
        let response = read(archive, selection, capture, cancel)?;
        checkpoint(cancel)?;
        watch.check()?;
        Ok(LocationPhysicsDescriptorSnapshot {
            build: archive.build().cloned(),
            status: response.status,
            descriptor: response.descriptor,
            present: response.present,
            grid: response.grid,
            source_inventory: capture.source_uses().freeze()?,
        })
    })
}

struct Read {
    status: LocationPhysicsDescriptorStatus,
    descriptor: Option<SourcedValue<PhysicalEntryRef>>,
    present: SourcedValue<bool>,
    grid: Option<Arc<LocationPhysicsGrid>>,
}

fn read(
    archive: &Archive,
    selection: LocationPhysicsDescriptorSelection,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
) -> Result<Read, CatalogError> {
    let directory = capture
        .source_uses()
        .observe_root_directory_paths(archive)?;
    let Some(entry) = archive.entries().iter().find(|entry| {
        entry.index() == (selection.container as usize)
            && crate::starmap_index::key(entry.path()).ends_with(".socpak")
    }) else {
        let complete =
            directory.is_some() && crate::export_archive_health::inventory(archive).is_none();
        let (status, value) = if complete {
            (
                LocationPhysicsDescriptorStatus::NotFound,
                SourceValue::Absent,
            )
        } else {
            (
                LocationPhysicsDescriptorStatus::Unavailable,
                SourceValue::Unreadable,
            )
        };
        return Ok(Read {
            status,
            descriptor: None,
            present: capture
                .source_uses()
                .value(value, directory.as_slice(), complete),
            grid: None,
        });
    };
    let mut ledger = ExportLedger::default();
    let Ok(main) = crate::starmap_read::main_xml(archive, entry, capture, &mut ledger)? else {
        let physical = EntrySource::outer(capture, archive, entry)?
            .receipt()
            .physical_reference(capture)?;
        return Ok(Read {
            status: LocationPhysicsDescriptorStatus::Unavailable,
            descriptor: None,
            present: capture.source_uses().value(
                SourceValue::Unreadable,
                physical.contributions().0,
                false,
            ),
            grid: None,
        });
    };
    let files = EntXmlFiles {
        archive,
        outer: entry,
        container: &main.container,
        capture,
        cancel,
    };
    read_main(
        &main.bytes,
        selection.descriptor,
        &files,
        directory.as_ref(),
    )
}

fn read_main(
    bytes: &EntryRead,
    descriptor: u32,
    files: &EntXmlFiles<'_, '_>,
    directory: Option<&SourceUse>,
) -> Result<Read, CatalogError> {
    if bytes.receipt().container_entry_index() != Some(descriptor as usize) {
        return attempted(
            bytes,
            files.capture,
            LocationPhysicsDescriptorStatus::SelectorMismatch,
        );
    }
    let Ok(xml) = EntryXml::parse_starmap_main(bytes, files.capture)? else {
        return attempted(
            bytes,
            files.capture,
            LocationPhysicsDescriptorStatus::Unavailable,
        );
    };
    let grid = match super::read(bytes, &xml, files, &OnceLock::new(), directory)? {
        GridRead::Unavailable => {
            return attempted(
                bytes,
                files.capture,
                LocationPhysicsDescriptorStatus::Unavailable,
            );
        }
        GridRead::Absent => None,
        GridRead::Present(row) => Some(row),
    };
    let sources = files.capture.source_uses();
    let descriptor = xml.payload_reference()?.into_cell(sources);
    if grid
        .as_ref()
        .is_some_and(|row| row.descriptor.value() != descriptor.value())
    {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let present = sources.value(
        SourceValue::Present(grid.is_some()),
        &[xml.document_source()?],
        true,
    );
    Ok(Read {
        status: LocationPhysicsDescriptorStatus::Read,
        descriptor: Some(descriptor),
        present,
        grid,
    })
}

fn attempted(
    bytes: &EntryRead,
    capture: &EntryCapture<'_>,
    status: LocationPhysicsDescriptorStatus,
) -> Result<Read, CatalogError> {
    let receipt = bytes.receipt();
    Ok(Read {
        status,
        descriptor: Some(
            receipt
                .payload_reference(capture)?
                .into_cell(capture.source_uses()),
        ),
        present: receipt.payload_read_result(SourceValue::Unreadable, false)?,
        grid: None,
    })
}
