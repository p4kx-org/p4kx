// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry as MapEntry;

use p4kx_extraction::{
    Archive, CancelToken, Entry, Guid, InputScope, SourceInput, SourceLocation, SourceUse,
    SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{
    EntryCapture, EntryDiagnosis, EntryRead, EntryXml, PhysicalEntryRef,
};
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::schema_fields::every_field_required;
use crate::starmap_index::key;
use crate::starmap_xml::Root;
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "starmap_hcd_document.rs"]
mod document;
pub use document::{HcdAttribute, HcdDocument};

const FLAG: &str = "hasHierarchicalContextData";
const GUID: &str = "guid";
const ROOT: &str = "HierarchicalContextData";
const PRESET: &str = "preset";

/// Lookup and read results for one authored location context declaration.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct HcdLink {
    failure: Option<SourcedValue<HcdSelectionFailure>>,
    candidates: SourcedValue<Vec<SourcedValue<PhysicalEntryRef>>>,
    document: SourcedValue<HcdDocument>,
}

impl HcdLink {
    /// Returns the typed context lookup failure.
    #[must_use]
    pub const fn failure(&self) -> Option<&SourcedValue<HcdSelectionFailure>> {
        self.failure.as_ref()
    }

    /// Returns the candidate members in directory order.
    #[must_use]
    pub const fn candidates(&self) -> &SourcedValue<Vec<SourcedValue<PhysicalEntryRef>>> {
        &self.candidates
    }

    /// Returns the selected context document.
    #[must_use]
    pub const fn document(&self) -> &SourcedValue<HcdDocument> {
        &self.document
    }
}

/// Why the original location context selector could not select one member.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum HcdSelectionFailure {
    /// The complete directory contains no matching member.
    Dangling,
    /// Several physical members match the selector.
    Ambiguous,
}

#[derive(Clone)]
enum HcdPayload {
    Readable {
        document: Box<HcdDocument>,
        root: SourceUse,
        complete: bool,
    },
    Unreadable,
    Invalid {
        root: Option<SourceUse>,
    },
}

/// Companion reads from the same sealed container as the placement descriptor.
pub(crate) struct HcdReader<'archive, 'capture, 'input> {
    files: HcdFiles<'archive, 'capture, 'input>,
    directory: Option<HcdDirectory<'archive>>,
    parsed: HashMap<Guid, HcdPayload>,
}

#[derive(Clone, Copy)]
pub(crate) enum HcdLookup {
    InvalidFlag,
    InvalidGuid,
    Guid(Guid),
}
struct HcdFiles<'archive, 'capture, 'input> {
    archive: &'archive Archive,
    outer: &'archive Entry,
    container: &'archive Archive,
    capture: &'capture EntryCapture<'input>,
    cancel: Option<&'archive CancelToken>,
}

struct HcdDirectory<'archive> {
    entries: HashMap<Guid, Vec<&'archive Entry>>,
    selection: SourceUse,
}

impl<'archive, 'capture, 'input> HcdReader<'archive, 'capture, 'input> {
    pub(crate) fn new(
        archive: &'archive Archive,
        outer: &'archive Entry,
        container: &'archive Archive,
        capture: &'capture EntryCapture<'input>,
        cancel: Option<&'archive CancelToken>,
    ) -> Self {
        Self {
            files: HcdFiles {
                archive,
                outer,
                container,
                capture,
                cancel,
            },

            parsed: HashMap::new(),
            directory: None,
        }
    }

    /// Reads a companion only when its owning Child declares the lookup.
    pub(crate) fn read(
        &mut self,
        xml: &EntryXml<'_, '_, '_>,
        child: Root<'_>,
        target: HcdLookup,
        ledger: &mut ExportLedger,
    ) -> Result<HcdLink, CatalogError> {
        let sources = self.files.capture.source_uses();
        let mut selection = Selection::new(xml, child, sources)?;
        if matches!(target, HcdLookup::InvalidFlag) {
            return Ok(selection.invalid());
        }
        let source = xml.attribute_source(child, GUID)?;
        selection
            .uses
            .push(source.observe(SourceUseRole::Conversion)?);
        let HcdLookup::Guid(guid) = target else {
            return Ok(selection.invalid());
        };
        let directory = match &mut self.directory {
            Some(directory) => directory,
            slot @ None => slot.insert(self.files.read_directory(xml)?),
        };
        selection.uses.push(directory.selection.clone());
        let entries = directory.entries.get(&guid).map_or(&[][..], Vec::as_slice);
        let candidates = entries
            .iter()
            .map(|entry| self.files.candidate(entry, &mut selection.uses))
            .collect::<Result<Vec<_>, _>>()?;
        let candidates = sources.value(
            SourceValue::Present(candidates),
            &selection.uses,
            selection.complete,
        );
        let [entry] = entries else {
            return Ok(selection.unresolved(candidates, !entries.is_empty()));
        };
        let payload = match self.parsed.entry(guid) {
            MapEntry::Occupied(held) => held.into_mut(),
            MapEntry::Vacant(slot) => slot.insert(self.files.read(entry, ledger)?),
        };
        Ok(selection.selected(candidates, payload))
    }
}

struct Selection<'source> {
    sources: &'source SourceUses,
    uses: Vec<SourceUse>,
    complete: bool,
}

impl<'source> Selection<'source> {
    fn new(
        xml: &EntryXml<'_, '_, '_>,
        child: Root<'_>,
        sources: &'source SourceUses,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            sources,
            uses: vec![
                xml.attribute_source(child, FLAG)?
                    .observe(SourceUseRole::Selector)?,
            ],
            complete: !xml.is_lossy(),
        })
    }

    fn invalid(&self) -> HcdLink {
        HcdLink {
            failure: None,
            candidates: self
                .sources
                .value(SourceValue::Absent, &self.uses, self.complete),
            document: self
                .sources
                .value(SourceValue::Invalid, &self.uses, self.complete),
        }
    }

    fn unresolved(
        &self,
        candidates: SourcedValue<Vec<SourcedValue<PhysicalEntryRef>>>,
        ambiguous: bool,
    ) -> HcdLink {
        let failure = if ambiguous {
            HcdSelectionFailure::Ambiguous
        } else {
            HcdSelectionFailure::Dangling
        };
        HcdLink {
            failure: Some(self.sources.value(
                SourceValue::Present(failure),
                &self.uses,
                self.complete,
            )),
            candidates,
            document: self
                .sources
                .value(SourceValue::Absent, &self.uses, self.complete),
        }
    }

    fn selected(
        mut self,
        candidates: SourcedValue<Vec<SourcedValue<PhysicalEntryRef>>>,
        payload: &HcdPayload,
    ) -> HcdLink {
        let document = match payload {
            HcdPayload::Readable {
                document,
                root,
                complete,
            } => {
                self.uses.push(root.clone());
                self.complete &= complete;
                SourceValue::Present(document.as_ref().clone())
            }
            HcdPayload::Unreadable => {
                self.complete = false;
                SourceValue::Unreadable
            }
            HcdPayload::Invalid { root } => {
                self.uses.extend(root.iter().cloned());
                if root.is_none() {
                    self.complete = false;
                }
                SourceValue::Invalid
            }
        };
        HcdLink {
            failure: None,
            candidates,
            document: self.sources.value(document, &self.uses, self.complete),
        }
    }
}

impl<'archive> HcdFiles<'archive, '_, '_> {
    fn read_directory(
        &self,
        xml: &EntryXml<'_, '_, '_>,
    ) -> Result<HcdDirectory<'archive>, CatalogError> {
        let selection = self.directory_survey(xml)?;
        let mut entries = HashMap::<Guid, Vec<&Entry>>::new();
        for entry in self.container.entries() {
            if self.cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            if let Some(guid) = companion_guid(entry.path()) {
                entries.entry(guid).or_default().push(entry);
            }
        }
        Ok(HcdDirectory { entries, selection })
    }
    fn check_owner(&self, xml: &EntryXml<'_, '_, '_>) -> Result<(), CatalogError> {
        let authored = xml.containing_entry()?.physical_reference(self.capture)?;
        let selected = EntrySource::outer(self.capture, self.archive, self.outer)?;
        let original = selected.receipt().physical_reference(self.capture)?;
        if authored.value() != original.value() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(())
    }

    fn directory_survey(&self, xml: &EntryXml<'_, '_, '_>) -> Result<SourceUse, CatalogError> {
        self.check_owner(xml)?;
        let entry = self
            .archive
            .observed_entry(self.outer)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let scope = self
            .capture
            .source_uses()
            .observe(SourceInput::ArchiveView(entry), InputScope::Payload)?;
        let admitted = self
            .container
            .container_input_origin()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if !admitted.binding().is_same_entry(&entry.binding()) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let count = u64::try_from(self.container.entries().len())
            .map_err(|_| ExportAccountingError::CounterOverflow)?;
        Ok(scope.at(
            SourceLocation::Subresource {
                namespace: "containerDirectory".to_owned(),
                indices: vec![0, count],
                field: "paths".to_owned(),
            },
            SourceUseRole::Selector,
        )?)
    }

    fn candidate(
        &self,
        entry: &Entry,
        selected: &mut Vec<SourceUse>,
    ) -> Result<SourcedValue<PhysicalEntryRef>, CatalogError> {
        let source = self.source(entry)?;
        let original = source.receipt().payload_reference(self.capture)?;
        selected.extend_from_slice(original.contributions().0);
        Ok(original.into_cell(self.capture.source_uses()))
    }

    fn source(&self, entry: &Entry) -> Result<EntrySource, CatalogError> {
        Ok(EntrySource::inner(
            self.capture,
            self.archive,
            self.outer,
            self.container,
            entry,
        )?
        .dependency())
    }

    fn read(&self, entry: &Entry, ledger: &mut ExportLedger) -> Result<HcdPayload, CatalogError> {
        let source = self.source(entry)?;
        let bytes =
            EntryRead::read_verified(source.receipt().clone(), self.container, entry, self.cancel)?;
        let Ok(bytes) = bytes else {
            source.record(ledger, Kind::StarmapObjects, SourceDisposition::ReadFailed);
            return Ok(HcdPayload::Unreadable);
        };
        let parsed = EntryXml::parse(&bytes, self.capture)?;
        let (body, diagnosis) = match parsed {
            Ok(xml) => HcdDocument::read(&xml, self.capture)?,
            Err(_) => (
                HcdPayload::Invalid { root: None },
                Some(ExportDiagnosticCode::MalformedSource),
            ),
        };
        if let Some(code) = diagnosis {
            source.diagnosed(
                ledger,
                Kind::StarmapObjects,
                SourceDisposition::ProjectionFailed,
                EntryDiagnosis::technical(code, PRESET, ROOT),
            );
        } else {
            source.record(ledger, Kind::StarmapObjects, SourceDisposition::Projected);
        }
        Ok(body)
    }
}

fn companion_guid(path: &str) -> Option<Guid> {
    let normalized = key(path);
    let mut parts = normalized.rsplit('\\');
    let stem = parts.next()?.strip_suffix(".xml")?;
    parts
        .any(|component| component == "hcd")
        .then_some(stem)
        .and_then(|stem| stem.parse().ok())
}

fn parse_guid(text: &str) -> Option<Guid> {
    let guid = text.trim().parse().ok()?;
    (guid != Guid::default()).then_some(guid)
}
