// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{
    Archive, CancelToken, Filter, Guid, SourceValue, SourceValueState, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_definition::DefinitionFamily;
use crate::catalog_read::loose_json::LooseJson;
use crate::catalog_read::original::{EntryCapture, PhysicalEntryRef};
use crate::catalog_read::parsed_entry_json;
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, Kind};

/// A declared terrain GUID relation and its original, ordered candidate receipts.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TerrainGuidLink {
    pub(crate) state: SourcedValue<TerrainGuidLinkState>,
    candidates: Vec<TerrainGuidCandidate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum TerrainGuidLinkState {
    Absent,
    Null,
    Invalid,
    Zero,
    Unreadable,
    Unsupported,
    Unavailable,
    Missing,
    WrongFamily,
    Ambiguous,
    Filtered,
    Resolved,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct TerrainGuidCandidate {
    kind: Kind,
    entry: SourcedValue<PhysicalEntryRef>,
    guid: SourcedValue<CatalogGuid>,
}

struct Observation {
    candidate: TerrainGuidCandidate,
    selected: bool,
}

/// The exact GUID identities read from the structurally eligible loose families.
pub(crate) struct TerrainGuidIndex {
    families: Vec<TerrainTargetFamily>,
    incomplete: HashSet<Kind>,
    directory: Option<p4kx_extraction::SourceUse>,
    candidates: HashMap<CatalogGuid, Vec<Observation>>,
}

#[derive(Clone, Copy)]
pub(crate) struct TerrainTargetFamily {
    pub(crate) kind: Kind,
    pub(crate) selector: DefinitionFamily,
    pub(crate) guid_field: &'static str,
}

impl TerrainGuidIndex {
    /// Reads wanted GUIDs from CRC-verified original definition Entries.
    /// Record positive matches even when the selected rows exclude the target Entry.
    pub(crate) fn read(
        archive: &Archive,
        capture: &EntryCapture<'_>,
        cancel: Option<&CancelToken>,
        filter: Option<&Filter>,
        families: Vec<TerrainTargetFamily>,
        wanted: &HashSet<CatalogGuid>,
    ) -> Result<Self, CatalogError> {
        let archive_complete = crate::export_archive_health::inventory(archive).is_none();
        let mut index = Self {
            incomplete: if archive_complete {
                HashSet::new()
            } else {
                families.iter().map(|family| family.kind).collect()
            },
            families,
            directory: None,
            candidates: HashMap::new(),
        };
        if wanted.is_empty() {
            return Ok(index);
        }
        index.read_candidates(archive, capture, cancel, filter, wanted)?;
        index.directory = capture
            .source_uses()
            .observe_root_directory_paths(archive)?;
        Ok(index)
    }

    fn read_candidates(
        &mut self,
        archive: &Archive,
        capture: &EntryCapture<'_>,
        cancel: Option<&CancelToken>,
        filter: Option<&Filter>,
        wanted: &HashSet<CatalogGuid>,
    ) -> Result<(), CatalogError> {
        for entry in archive.entries() {
            parsed_entry_json::check_cancel(cancel)?;
            let Some(target) = self
                .families
                .iter()
                .copied()
                .find(|family| family.selector.includes(entry.path()))
            else {
                continue;
            };
            let family = target.kind;
            let source = EntrySource::outer(capture, archive, entry)?.dependency();
            let Ok(read) = LooseJson::read(archive, entry, &source, cancel)? else {
                self.incomplete.insert(family);
                continue;
            };
            let Ok(parsed) = read.parse(capture, cancel)? else {
                self.incomplete.insert(family);
                continue;
            };
            let Some(object) = EntryJsonObject::new(&parsed, parsed.root, capture.source_uses())?
            else {
                self.incomplete.insert(family);
                continue;
            };
            let guid = object.field(target.guid_field, entry_json_object::guid)?;
            let Some(&identity) = guid.value() else {
                self.incomplete.insert(family);
                continue;
            };
            if !wanted.contains(&identity) {
                continue;
            }
            let reference = source
                .receipt()
                .physical_reference(capture)?
                .into_cell(capture.source_uses());
            let observation = Observation {
                candidate: TerrainGuidCandidate {
                    kind: family,
                    entry: reference,
                    guid,
                },
                selected: filter.is_none_or(|filter| filter.matches(entry.path())),
            };
            self.candidates
                .try_reserve(1)
                .map_err(CatalogError::JsonSourceAllocation)?;
            parsed_entry_json::push(self.candidates.entry(identity).or_default(), observation)?;
        }
        Ok(())
    }

    /// Correlates a declared field with its original GUID and physical Entry candidates.
    /// Negative and uniqueness evidence remains incomplete if nonmatches are not cited.
    pub(crate) fn resolve(
        &self,
        family: Kind,
        selector: &SourcedValue<CatalogGuid>,
        capture: &EntryCapture<'_>,
    ) -> Result<TerrainGuidLink, CatalogError> {
        let sources = capture.source_uses();
        let observations = selector
            .value()
            .and_then(|guid| self.candidates.get(guid))
            .map_or(&[][..], Vec::as_slice);
        let mut selected = Vec::new();
        let mut matching = 0;
        let mut included = false;
        let mut origins = vec![selector.origin()];
        for item in observations {
            if family == item.candidate.kind {
                matching += 1;
                included |= item.selected;
            }
            origins.extend([item.candidate.guid.origin(), item.candidate.entry.origin()]);
            parsed_entry_json::push(&mut selected, item.candidate.clone())?;
        }
        let state = match (selector.state(), selector.value()) {
            (SourceValueState::Absent, _) => TerrainGuidLinkState::Absent,
            (SourceValueState::Null, _) => TerrainGuidLinkState::Null,
            (SourceValueState::Unreadable, _) => TerrainGuidLinkState::Unreadable,
            (SourceValueState::Unsupported, _) => TerrainGuidLinkState::Unsupported,
            (SourceValueState::Invalid, _) | (_, None) => TerrainGuidLinkState::Invalid,
            (_, Some(guid)) if guid.0 == Guid::default() => TerrainGuidLinkState::Zero,
            (_, Some(_)) if matching > 1 => TerrainGuidLinkState::Ambiguous,
            (_, Some(_))
                if self.incomplete.contains(&family)
                    || !self.families.iter().any(|known| known.kind == family) =>
            {
                TerrainGuidLinkState::Unavailable
            }
            (_, Some(_)) if matching == 1 && included => TerrainGuidLinkState::Resolved,
            (_, Some(_)) if matching == 1 => TerrainGuidLinkState::Filtered,
            (_, Some(_)) if !observations.is_empty() => TerrainGuidLinkState::WrongFamily,
            (_, Some(_)) => TerrainGuidLinkState::Missing,
        };
        // Nonmatching payloads are outside this edge's recorded origin.
        let survey = sources.origin(self.directory.as_slice(), false);
        if matches!(
            state,
            TerrainGuidLinkState::Unavailable
                | TerrainGuidLinkState::Missing
                | TerrainGuidLinkState::WrongFamily
                | TerrainGuidLinkState::Filtered
                | TerrainGuidLinkState::Resolved
        ) {
            origins.push(&survey);
        }
        Ok(TerrainGuidLink {
            state: sources.value_with_origins(SourceValue::Present(state), &origins)?,
            candidates: selected,
        })
    }
}

pub(crate) const fn link_issue(state: TerrainGuidLinkState) -> Option<ExportDiagnosticCode> {
    match state {
        TerrainGuidLinkState::Unreadable => Some(ExportDiagnosticCode::UnreadableSource),
        TerrainGuidLinkState::Unavailable | TerrainGuidLinkState::Filtered => {
            Some(ExportDiagnosticCode::CatalogTargetUnavailable)
        }
        TerrainGuidLinkState::Missing => Some(ExportDiagnosticCode::DanglingReference),
        TerrainGuidLinkState::WrongFamily => Some(ExportDiagnosticCode::MalformedSource),
        TerrainGuidLinkState::Ambiguous => Some(ExportDiagnosticCode::AmbiguousIdentity),
        TerrainGuidLinkState::Unsupported => Some(ExportDiagnosticCode::UnsupportedValue),
        TerrainGuidLinkState::Absent
        | TerrainGuidLinkState::Null
        | TerrainGuidLinkState::Invalid
        | TerrainGuidLinkState::Zero
        | TerrainGuidLinkState::Resolved => None,
    }
}
