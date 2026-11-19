// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, ArchiveWatch, CancelToken, Entry, Filter, SourceUse, SourceValue, SourcedValue,
};

use crate::catalog_read::authored_xml_tree as tree;
use crate::catalog_read::original::{
    EntryCapture, EntryDiagnosis, EntryRead, EntrySourceXml, with_entry_capture,
};
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::{CatalogError, Kind, starmap_index};

#[path = "client_profile_document.rs"]
mod document;

pub use self::document::{
    ClientProfileSettings, ClientProfileSettingsReport, ProfileDocument, ProfileKind, ProfileState,
};
pub use crate::catalog_read::authored_xml_tree::{
    AuthoredXmlDeclaration as ProfileDeclaration, AuthoredXmlElement as ProfileElement,
    AuthoredXmlText as ProfileText,
};
pub use crate::catalog_read::original::PhysicalEntryIdentity;

const KIND: Kind = Kind::ClientProfileSettings;
const GENERAL: &str = "Data/Libs/Config/Profiles/default/attributes.xml";
const VR: &str = "Data/Libs/Config/Profiles/default/attributesVR.xml";

/// Reads the authored client profile documents in one root Archive.
///
/// # Errors
/// Returns an error if the Archive changes, source accounting fails or the caller cancels.
pub fn read_client_profile_settings(
    archive: &Archive,
    cancel: Option<&CancelToken>,
) -> Result<ClientProfileSettingsReport, CatalogError> {
    read_profile_report(archive, |_| true, cancel)
}

/// Reads one profile's payload and retains both directory selection outcomes.
///
/// # Errors
/// Returns the same errors as [`read_client_profile_settings`].
pub fn read_client_profile_settings_for(
    archive: &Archive,
    kind: ProfileKind,
    cancel: Option<&CancelToken>,
) -> Result<ClientProfileSettingsReport, CatalogError> {
    read_profile_report(archive, |candidate| candidate == kind, cancel)
}

fn read_profile_report(
    archive: &Archive,
    selected: impl Fn(ProfileKind) -> bool,
    cancel: Option<&CancelToken>,
) -> Result<ClientProfileSettingsReport, CatalogError> {
    check_cancel(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let report = with_entry_capture(archive, |capture| {
        let settings = read_selected(
            archive,
            capture,
            selected,
            cancel,
            &mut ExportLedger::default(),
        )?;
        let provenance = capture.source_uses().finish_with(|| check_cancel(cancel))?;
        Ok(ClientProfileSettingsReport {
            settings,
            provenance,
            build: archive.build().cloned(),
        })
    });
    watch.check()?;
    check_cancel(cancel)?;
    report
}

pub(crate) fn read(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    ledger: &mut ExportLedger,
) -> Result<ClientProfileSettings, CatalogError> {
    read_selected(
        archive,
        capture,
        |kind| filter.is_none_or(|filter| filter.matches(kind.path())),
        cancel,
        ledger,
    )
}

fn read_selected(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    selected: impl Fn(ProfileKind) -> bool,
    cancel: Option<&CancelToken>,
    ledger: &mut ExportLedger,
) -> Result<ClientProfileSettings, CatalogError> {
    ledger.inventory(KIND);
    let mut general = Vec::new();
    let mut vr = Vec::new();
    for entry in archive.entries() {
        check_cancel(cancel)?;
        if same_path(entry.path(), GENERAL) {
            general.push(entry);
        } else if same_path(entry.path(), VR) {
            vr.push(entry);
        }
    }
    let directory = capture
        .source_uses()
        .observe_root_directory_paths(archive)?;
    let complete =
        directory.is_some() && crate::export_archive_health::inventory(archive).is_none();
    let inputs = Inputs {
        archive,
        capture,
        directory: directory.as_slice(),
        complete,
        cancel,
    };
    let general = inputs.document(
        ProfileKind::General,
        &general,
        selected(ProfileKind::General),
        ledger,
    )?;
    let vr = inputs.document(
        ProfileKind::VirtualReality,
        &vr,
        selected(ProfileKind::VirtualReality),
        ledger,
    )?;
    Ok(ClientProfileSettings {
        documents: [general, vr],
    })
}

struct Inputs<'a, 'input> {
    archive: &'a Archive,
    capture: &'a EntryCapture<'input>,
    directory: &'a [SourceUse],
    complete: bool,
    cancel: Option<&'a CancelToken>,
}

impl Inputs<'_, '_> {
    fn document(
        &self,
        kind: ProfileKind,
        entries: &[&Entry],
        selected: bool,
        ledger: &mut ExportLedger,
    ) -> Result<ProfileDocument, CatalogError> {
        let candidates = entries
            .iter()
            .map(|entry| {
                self.entry_source(entry)?
                    .receipt()
                    .physical_payload_file(starmap_index::key)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (status, elements) = match entries {
            _ if !selected => (
                self.directory_status(ProfileState::Filtered, &candidates)?,
                Vec::new(),
            ),
            [entry] if self.complete && self.archive.raw_path(entry.index()).is_none() => {
                self.decode(entry, ledger)?
            }
            _ if entries.len() > 1 => {
                for entry in entries {
                    self.entry_source(entry)?.selection_candidate().diagnosed(
                        ledger,
                        KIND,
                        SourceDisposition::ReadFailed,
                        diagnosis(ExportDiagnosticCode::AmbiguousIdentity),
                    );
                }
                (
                    self.directory_status(ProfileState::Ambiguous, &candidates)?,
                    Vec::new(),
                )
            }
            [] if self.complete => (
                self.directory_status(ProfileState::Missing, &candidates)?,
                Vec::new(),
            ),
            _ => {
                ledger.incomplete_inventory(KIND);
                (
                    self.directory_status(ProfileState::Unavailable, &candidates)?,
                    Vec::new(),
                )
            }
        };
        Ok(ProfileDocument {
            kind,
            requested_path: kind.path(),
            status,
            candidates,
            elements,
        })
    }

    fn directory_status(
        &self,
        state: ProfileState,
        candidates: &[PhysicalEntryIdentity],
    ) -> Result<SourcedValue<ProfileState>, CatalogError> {
        let sources = self.capture.source_uses();
        let directory = sources.origin(self.directory, self.complete);
        let mut origins = vec![&directory];
        origins.extend(
            candidates
                .iter()
                .map(|candidate| candidate.source_path.origin()),
        );
        Ok(sources.value_with_origins(SourceValue::Present(state), &origins)?)
    }

    fn entry_source(&self, entry: &Entry) -> Result<EntrySource, CatalogError> {
        Ok(EntrySource::outer(self.capture, self.archive, entry)?
            .operation(EntryOperation::SourceXml))
    }

    fn decode(
        &self,
        entry: &Entry,
        ledger: &mut ExportLedger,
    ) -> Result<(SourcedValue<ProfileState>, Vec<ProfileElement>), CatalogError> {
        let source = self.entry_source(entry)?;
        let receipt = source.receipt();
        let (state, elements) =
            match EntryRead::read_verified(receipt.clone(), self.archive, entry, self.cancel)? {
                Err(_) => (ProfileState::Unreadable, Vec::new()),
                Ok(read) => match EntrySourceXml::parse(&read, self.capture)? {
                    Err(_) => (ProfileState::Malformed, Vec::new()),
                    Ok(xml) if xml.is_lossy() => (ProfileState::Malformed, Vec::new()),
                    Ok(xml) if xml.root().name() != "Attributes" => {
                        (ProfileState::Unsupported, Vec::new())
                    }
                    Ok(xml) => (ProfileState::Read, tree::read(&xml, self.cancel)?),
                },
            };
        let status = receipt.payload_read_result(
            SourceValue::Present(state),
            state != ProfileState::Unreadable,
        )?;
        record(source, state, ledger);
        Ok((status, elements))
    }
}

fn record(source: EntrySource, state: ProfileState, ledger: &mut ExportLedger) {
    let (disposition, problem) = match state {
        ProfileState::Read => {
            source.record(ledger, KIND, SourceDisposition::Projected);
            return;
        }
        ProfileState::Unreadable => (
            SourceDisposition::ReadFailed,
            ExportDiagnosticCode::SourceReadFailure,
        ),
        ProfileState::Unsupported => (
            SourceDisposition::ProjectionFailed,
            ExportDiagnosticCode::UnsupportedValue,
        ),
        _ => (
            SourceDisposition::ProjectionFailed,
            ExportDiagnosticCode::MalformedSource,
        ),
    };
    source.diagnosed(ledger, KIND, disposition, diagnosis(problem));
}

const fn diagnosis(code: ExportDiagnosticCode) -> EntryDiagnosis {
    EntryDiagnosis::technical(code, "content", "Attributes")
}

fn same_path(actual: &str, expected: &str) -> bool {
    actual.len() == expected.len()
        && actual.bytes().zip(expected.bytes()).all(|(left, right)| {
            (left == b'\\' && right == b'/') || left.eq_ignore_ascii_case(&right)
        })
}

fn check_cancel(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}

#[cfg(test)]
#[path = "client_profile_settings_tests.rs"]
mod tests;
