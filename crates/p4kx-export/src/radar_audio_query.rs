// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to authored radar-audio configuration.

use std::slice;

pub use p4kx_extraction::sound::radar_audio::{RadarAudioElement, RadarAudioField};
use p4kx_extraction::{
    Archive, ArchiveWatch, Build, CancelToken, Entry, FieldOrigin, ProvenanceMode, SourceInventory,
};

use crate::catalog_query_session::checkpoint;
pub use crate::catalog_read::authored_xml_tree::{
    AuthoredXmlDeclaration, AuthoredXmlElement, AuthoredXmlText,
};
use crate::catalog_read::original::{EntryCapture, EntryReceipt, with_entry_capture_mode};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::radar_audio_configuration;
pub use crate::catalog_v3::radar_audio_configuration::{
    RadarAudioBindingStatus, RadarAudioCell, RadarAudioConfiguration, RadarAudioDatum,
    RadarAudioNode,
};
use crate::export_archive_health;
use crate::export_entry_counts::ExportEntryCounts;
use crate::export_ledger::ExportLedger;
use crate::{CatalogError, export_health_entries};

/// Selects physical radar-audio configuration Entries in one Archive.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum RadarAudioSelection {
    /// Every physical occurrence at the canonical radar-audio path.
    All,
    /// The occurrence at the specified root directory index.
    EntryIndex(u32),
}

/// Canonical radar-audio rows with physical source evidence and admission coverage.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct RadarAudioSnapshot {
    /// The Build identified by the Archive, when available.
    pub build: Option<Build>,
    /// Availability of the configuration within the selection.
    pub availability: DatasetAvailability,
    /// Configuration rows in physical directory order.
    pub rows: Vec<RadarAudioConfiguration>,
    /// The source scope used to select physical Entries.
    pub directory: FieldOrigin,
    /// Read and projection outcomes for the selected Entries.
    pub entries: ExportEntryCounts,
    /// Sources and uses referenced by this selection.
    pub source_inventory: SourceInventory,
}

/// Reads the original radar-audio configuration without opening DataForge or soundbanks.
///
/// Required Entry read and parse failures are counted in the snapshot. An Entry index belongs
/// to the currently opened root Archive.
///
/// # Errors
/// Returns an error for cancellation, an Archive change or a source-accounting fault.
pub fn read_radar_audio(
    archive: &Archive,
    selection: RadarAudioSelection,
    cancel: Option<&CancelToken>,
) -> Result<RadarAudioSnapshot, CatalogError> {
    read_radar_audio_with_provenance_mode(archive, selection, ProvenanceMode::MarkersOnly, cancel)
}

/// Reads radar audio with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_radar_audio`.
pub fn read_radar_audio_with_provenance_mode(
    archive: &Archive,
    selection: RadarAudioSelection,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<RadarAudioSnapshot, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let snapshot = with_entry_capture_mode(archive, provenance_mode, |capture| {
        let inventory_problem = export_archive_health::inventory(archive);
        let mut ledger = ExportLedger {
            archive_inventory_problem: inventory_problem,
            ..ExportLedger::default()
        };
        let entries = match selection {
            RadarAudioSelection::All => archive.entries(),
            RadarAudioSelection::EntryIndex(index) => usize::try_from(index)
                .ok()
                .and_then(|index| archive.entries().get(index))
                .map_or(&[][..], slice::from_ref),
        };
        let table =
            radar_audio_configuration::read(archive, entries, capture, None, cancel, &mut ledger)?;
        let registry = capture.source_uses();
        let directory = directory_origin(
            archive,
            capture,
            selection,
            entries,
            inventory_problem.is_none(),
        )?;
        let observed = export_health_entries::compile(ledger.entries)?;
        let entries =
            export_health_entries::counts(radar_audio_configuration::KIND, &observed.sources)?;
        let source_inventory = registry.finish_with(|| checkpoint(cancel))?;
        Ok(RadarAudioSnapshot {
            build: archive.build().cloned(),
            availability: table.availability,
            rows: table.rows,
            directory,
            entries,
            source_inventory,
        })
    });
    watch.check()?;
    checkpoint(cancel)?;
    snapshot
}

fn directory_origin(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    selection: RadarAudioSelection,
    entries: &[Entry],
    inventory_complete: bool,
) -> Result<FieldOrigin, CatalogError> {
    let registry = capture.source_uses();
    let proof = match selection {
        RadarAudioSelection::EntryIndex(_) => {
            if let [entry] = entries {
                let receipt = EntryReceipt::outer(capture, archive, entry)?;
                let reference = receipt.physical_reference(capture)?;
                let (sources, complete) = reference.contributions();
                return Ok(registry.origin(sources, complete));
            }
            registry.observe_root_directory_entry_count(archive)?
        }
        RadarAudioSelection::All => registry.observe_root_directory_paths(archive)?,
    };
    Ok(registry.origin(proof.as_slice(), proof.is_some() && inventory_complete))
}
