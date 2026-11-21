// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

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
use crate::starmap_xml::{Node, Root};
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "starmap_entxml_document.rs"]
mod document;
pub use document::{AreaEntity, EntXmlDocument, HarvestableComponent};
use document::{ENTITY, ENTITY_GUID, parse_guid};

const CHILD_GUID: &str = "guid";

/// Results of the same-container entity lookup for this original Child.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct EntXmlLink {
    candidates: SourcedValue<Vec<EntXmlCandidate>>,
    inventory_complete: bool,
    selected: SourcedValue<PhysicalEntryRef>,
}

impl EntXmlLink {
    /// Returns matched entity candidates in directory order.
    #[must_use]
    pub const fn candidates(&self) -> &SourcedValue<Vec<EntXmlCandidate>> {
        &self.candidates
    }

    /// Reports whether the same-container lookup completed.
    #[must_use]
    pub const fn inventory_complete(&self) -> bool {
        self.inventory_complete
    }

    /// Returns the uniquely selected physical member.
    #[must_use]
    pub const fn selected(&self) -> &SourcedValue<PhysicalEntryRef> {
        &self.selected
    }
}

/// One physical member that contains the requested entity declaration.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub struct EntXmlCandidate {
    entry: SourcedValue<PhysicalEntryRef>,
    document: SourcedValue<EntXmlDocument>,
}

impl EntXmlCandidate {
    /// Returns the physical member identity.
    #[must_use]
    pub const fn entry(&self) -> &SourcedValue<PhysicalEntryRef> {
        &self.entry
    }

    /// Returns the authored entity document.
    #[must_use]
    pub const fn document(&self) -> &SourcedValue<EntXmlDocument> {
        &self.document
    }
}
/// The original Child selector and the identity bucket it may select.
#[derive(Clone, Debug)]
pub(crate) struct EntXmlRequest {
    guid: Option<Guid>,
    selector: SourceUse,
    complete: bool,
}

impl EntXmlRequest {
    pub(crate) fn capture(
        xml: &EntryXml<'_, '_, '_>,
        child: Root<'_>,
        guid: Option<Guid>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            guid: guid.filter(|guid| *guid != Guid::default()),
            selector: xml
                .attribute_source(child, CHILD_GUID)?
                .observe(SourceUseRole::Selector)?,
            complete: !xml.is_lossy(),
        })
    }

    pub(crate) const fn guid(&self) -> Option<Guid> {
        self.guid
    }
}

struct Candidate {
    row: EntXmlCandidate,
    root_selector: SourceUse,
}

/// Members of one original physical container, with only requested identities retained.
pub(crate) struct EntXmlIndex {
    complete: bool,
    selected: HashMap<Guid, Vec<Candidate>>,
    surveyed_paths: Option<SourceUse>,
}

pub(crate) struct EntXmlFiles<'a, 'input> {
    pub(crate) archive: &'a Archive,
    pub(crate) outer: &'a Entry,
    pub(crate) container: &'a Archive,
    pub(crate) capture: &'a EntryCapture<'input>,
    pub(crate) cancel: Option<&'a CancelToken>,
}

impl EntXmlFiles<'_, '_> {
    pub(crate) fn scan(
        &self,
        wanted: &HashSet<Guid>,
        descriptor: &EntryXml<'_, '_, '_>,
        ledger: &mut ExportLedger,
    ) -> Result<EntXmlIndex, CatalogError> {
        let mut index = EntXmlIndex {
            complete: true,
            selected: HashMap::new(),
            surveyed_paths: None,
        };
        if wanted.is_empty() {
            return Ok(index);
        }
        let outer = self
            .archive
            .observed_entry(self.outer)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let binding = self
            .container
            .container_input_origin()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let original = EntrySource::outer(self.capture, self.archive, self.outer)?
            .receipt()
            .physical_reference(self.capture)?;
        let declaring = descriptor
            .containing_entry()?
            .physical_reference(self.capture)?;
        if !binding.binding().is_same_entry(&outer.binding())
            || original.value() != declaring.value()
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        for entry in self.container.entries() {
            if self.cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            if selected_entry(entry.path()) {
                index.complete &= self.read_member(entry, wanted, &mut index.selected, ledger)?;
            }
        }
        let scope = self
            .capture
            .source_uses()
            .observe(SourceInput::ArchiveView(outer), InputScope::Payload)?;
        let count = u64::try_from(self.container.entries().len())
            .map_err(|_| ExportAccountingError::CounterOverflow)?;
        index.surveyed_paths = Some(scope.at(
            SourceLocation::Subresource {
                namespace: "containerDirectory".to_owned(),
                indices: vec![0, count],
                field: "paths".to_owned(),
            },
            SourceUseRole::Selector,
        )?);
        Ok(index)
    }

    fn read_member(
        &self,
        entry: &Entry,
        wanted: &HashSet<Guid>,
        selected: &mut HashMap<Guid, Vec<Candidate>>,
        ledger: &mut ExportLedger,
    ) -> Result<bool, CatalogError> {
        let source = EntrySource::inner(
            self.capture,
            self.archive,
            self.outer,
            self.container,
            entry,
        )?
        .dependency();
        let Ok(bytes) =
            EntryRead::read_verified(source.receipt().clone(), self.container, entry, self.cancel)?
        else {
            source.record(ledger, Kind::StarmapObjects, SourceDisposition::ReadFailed);
            return Ok(false);
        };
        let Ok(xml) = EntryXml::parse(&bytes, self.capture)? else {
            return Ok(failed_projection(
                ledger,
                source,
                ExportDiagnosticCode::MalformedSource,
            ));
        };
        if xml.is_lossy() || xml.root().name() != ENTITY {
            return Ok(failed_projection(
                ledger,
                source,
                ExportDiagnosticCode::UnsupportedValue,
            ));
        }
        let mut ids = xml
            .root()
            .attributes()
            .filter(|(name, _)| name.eq_ignore_ascii_case(ENTITY_GUID));
        let guid = ids.next().and_then(|(_, raw)| parse_guid(raw));
        let Some(guid) = guid.filter(|_| ids.next().is_none()) else {
            return Ok(failed_projection(
                ledger,
                source,
                ExportDiagnosticCode::MalformedSource,
            ));
        };
        if wanted.contains(&guid) {
            let (document, root_selector) = EntXmlDocument::project(&xml)?;
            let reference = source
                .receipt()
                .payload_reference(self.capture)?
                .into_cell(self.capture.source_uses());
            selected.entry(guid).or_default().push(Candidate {
                row: EntXmlCandidate {
                    entry: reference,
                    document: self.capture.source_uses().value(
                        SourceValue::Present(document),
                        std::slice::from_ref(&root_selector),
                        true,
                    ),
                },
                root_selector,
            });
        }
        source.record(ledger, Kind::StarmapObjects, SourceDisposition::Projected);
        Ok(true)
    }
}

fn failed_projection(
    ledger: &mut ExportLedger,
    source: EntrySource,
    code: ExportDiagnosticCode,
) -> bool {
    source.diagnosed(
        ledger,
        Kind::StarmapObjects,
        SourceDisposition::ProjectionFailed,
        EntryDiagnosis::technical(code, ENTITY_GUID, ENTITY),
    );
    false
}

impl EntXmlIndex {
    pub(crate) fn project(&self, request: &EntXmlRequest, sources: &SourceUses) -> EntXmlLink {
        let uses = [request.selector.clone()];
        let Some(guid) = request.guid else {
            return EntXmlLink {
                candidates: sources.value(SourceValue::Invalid, &uses, request.complete),
                inventory_complete: false,
                selected: sources.value(SourceValue::Invalid, &uses, request.complete),
            };
        };
        let matches = self.selected.get(&guid).map_or(&[][..], Vec::as_slice);
        let mut candidate_uses = uses.to_vec();
        candidate_uses.extend(self.surveyed_paths.iter().cloned());
        candidate_uses.extend(
            matches
                .iter()
                .map(|candidate| candidate.root_selector.clone()),
        );
        let candidate_refs = matches
            .iter()
            .map(|candidate| candidate.row.clone())
            .collect();
        let exact_set = self.complete && request.complete;
        // Directory paths do not prove the contents of unmatched members.
        let candidates =
            sources.value(SourceValue::Present(candidate_refs), &candidate_uses, false);
        let selected = match matches {
            [candidate] if exact_set => sources.value(
                candidate
                    .row
                    .entry
                    .value()
                    .cloned()
                    .map_or(SourceValue::Invalid, SourceValue::Present),
                &candidate_uses,
                false,
            ),
            [] if exact_set => sources.value(SourceValue::Absent, &candidate_uses, false),
            [_, _, ..] => sources.value(SourceValue::Invalid, &candidate_uses, false),
            _ => sources.value(SourceValue::Unreadable, &candidate_uses, false),
        };
        EntXmlLink {
            candidates,
            inventory_complete: exact_set,
            selected,
        }
    }
}

fn selected_entry(path: &str) -> bool {
    let mut segments = path.rsplit(['/', '\\']);
    let Some(leaf) = segments.next() else {
        return false;
    };
    leaf.rsplit_once('.')
        .is_some_and(|(_, suffix)| suffix.eq_ignore_ascii_case("entxml"))
        && segments.any(|segment| segment.eq_ignore_ascii_case("entdata"))
}
