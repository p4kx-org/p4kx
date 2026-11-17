// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, CancelToken, Entry, Filter, SourceInventory, SourceUse, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{
    EntryCapture, EntryDiagnosis, EntryRead, EntrySourceXml, PhysicalEntryIdentity,
    with_entry_capture,
};
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::schema_fields::every_field_required;
use crate::{CatalogError, Kind, starmap_index};

#[path = "client_profile_tree.rs"]
mod tree;

use self::tree::ProfileElement;

const KIND: Kind = Kind::ClientProfileSettings;
const GENERAL: &str = "Data/Libs/Config/Profiles/default/attributes.xml";
const VR: &str = "Data/Libs/Config/Profiles/default/attributesVR.xml";

#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum ProfileKind {
    General,
    VirtualReality,
}

impl ProfileKind {
    const fn path(self) -> &'static str {
        match self {
            Self::General => GENERAL,
            Self::VirtualReality => VR,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum ProfileState {
    Read,
    Missing,
    Filtered,
    Unavailable,
    Ambiguous,
    Unreadable,
    Malformed,
    Unsupported,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ProfileDocument {
    kind: ProfileKind,
    requested_path: &'static str,
    status: SourcedValue<ProfileState>,
    candidates: Vec<PhysicalEntryIdentity>,
    elements: Vec<ProfileElement>,
}

/// The authored General and VR client profile documents.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ClientProfileSettings {
    documents: [ProfileDocument; 2],
}

impl ClientProfileSettings {
    /// Returns whether a document could not be selected or decoded safely.
    #[must_use]
    pub fn has_failed_inputs(&self) -> bool {
        self.documents.iter().any(|document| {
            !matches!(
                document.status.value(),
                Some(ProfileState::Read | ProfileState::Missing | ProfileState::Filtered)
            )
        })
    }

    /// Returns the number of documents successfully decoded.
    #[must_use]
    pub fn decoded_documents(&self) -> usize {
        self.documents
            .iter()
            .filter(|document| document.status.value() == Some(&ProfileState::Read))
            .count()
    }
}

/// A standalone client profile result with its complete source inventory.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ClientProfileSettingsReport {
    settings: ClientProfileSettings,
    provenance: SourceInventory,
}

impl ClientProfileSettingsReport {
    /// Returns the observed profile documents.
    #[must_use]
    pub const fn settings(&self) -> &ClientProfileSettings {
        &self.settings
    }

    /// Returns the inventory addressed by the profile fields.
    #[must_use]
    pub const fn provenance(&self) -> &SourceInventory {
        &self.provenance
    }
}

/// Reads the authored client profile documents in one root Archive.
///
/// # Errors
/// Returns an error if source accounting fails or the caller cancels the operation.
pub fn read_client_profile_settings(
    archive: &Archive,
    cancel: Option<&CancelToken>,
) -> Result<ClientProfileSettingsReport, CatalogError> {
    with_entry_capture(archive, |capture| {
        let settings = read(archive, capture, None, cancel, &mut ExportLedger::default())?;
        Ok(ClientProfileSettingsReport {
            settings,
            provenance: capture.source_uses().freeze()?,
        })
    })
}

pub(crate) fn read(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    filter: Option<&Filter>,
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
        filter,
        cancel,
    };
    Ok(ClientProfileSettings {
        documents: [
            inputs.document(ProfileKind::General, &general, ledger)?,
            inputs.document(ProfileKind::VirtualReality, &vr, ledger)?,
        ],
    })
}

struct Inputs<'a, 'input> {
    archive: &'a Archive,
    capture: &'a EntryCapture<'input>,
    directory: &'a [SourceUse],
    complete: bool,
    filter: Option<&'a Filter>,
    cancel: Option<&'a CancelToken>,
}

impl Inputs<'_, '_> {
    fn document(
        &self,
        kind: ProfileKind,
        entries: &[&Entry],
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
            _ if self
                .filter
                .is_some_and(|filter| !filter.matches(kind.path())) =>
            {
                (
                    self.directory_status(ProfileState::Filtered, &candidates)?,
                    Vec::new(),
                )
            }
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
