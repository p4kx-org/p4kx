// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use anyhow::Result;
use p4kx_assembly::{
    LoadoutClassSelection, LoadoutFileObservation, LoadoutPath, LoadoutReferenceSelection,
    LoadoutSelection,
};
use p4kx_extraction::{
    FieldOrigin, Guid, InputScope, SourceCoordinateOmission, SourceInput, SourceLocation,
    SourceUseRole,
};

use super::Output;
use super::definition_sites::{Definition, DefinitionKey};
use super::record_origins::{Datum, RecordName, RecordSite, Selection};
use super::vehicle_selection_origins::VehicleFacts;

#[derive(Default)]
pub(super) struct Facts {
    pub(super) paths: HashMap<LoadoutSelection, Authored>,
    pub(super) classes: HashMap<LoadoutClassSelection, Authored>,
    pub(super) references: HashMap<LoadoutReferenceSelection, RecordSite>,
    pub(super) class_records: HashMap<LoadoutClassSelection, Option<u32>>,
    pub(super) definitions: HashMap<DefinitionKey, Definition>,
    pub(super) vehicles: VehicleFacts,
    pub(super) derived: HashMap<LoadoutSelection, DerivedPath>,
}

impl Facts {
    pub(super) fn retain(&mut self, selection: Selection, site: RecordSite) {
        match selection {
            Selection::Path(selection) => {
                self.paths.insert(selection, Authored::Record(site));
            }
            Selection::Class(selection) => {
                self.classes.insert(selection, Authored::Record(site));
            }
            Selection::Reference(selection) => {
                self.references.insert(selection, site);
            }
        }
    }

    pub(super) fn class_record(&mut self, selection: &LoadoutClassSelection, record: u32) {
        self.class_records
            .entry(selection.clone())
            .and_modify(|retained| {
                if *retained != Some(record) {
                    *retained = None;
                }
            })
            .or_insert(Some(record));
    }

    pub(super) fn class_was_selected(
        &self,
        selection: &LoadoutClassSelection,
        record: u32,
    ) -> bool {
        self.class_records.get(selection).copied().flatten() == Some(record)
    }
}

pub(super) enum Authored {
    Record(RecordSite),
    RecordName(RecordName),
    Xml(Xml),
}

pub(super) struct DerivedPath {
    pub(super) value: String,
    pub(super) inputs: Vec<LoadoutPath>,
}

pub(super) struct Xml {
    pub(super) file: LoadoutFileObservation,
    pub(super) location: Option<SourceLocation>,
}

impl Output<'_, '_> {
    pub(super) fn path_origin(
        &self,
        path: Option<&LoadoutPath>,
        role: SourceUseRole,
    ) -> Result<FieldOrigin> {
        let Some((path, selection)) = path.and_then(|path| Some((path, path.selection()?))) else {
            return Ok(self.uses.origin(&[], false));
        };
        if let Some(derived) = self
            .facts
            .derived
            .get(selection)
            .filter(|derived| derived.value == path.value())
        {
            let inputs = derived
                .inputs
                .iter()
                .map(|path| self.path_origin(Some(path), role))
                .collect::<Result<Vec<_>>>()?;
            return Ok(self.uses.origin_with_origins(&inputs, !inputs.is_empty())?);
        }
        self.authored(self.facts.paths.get(selection), role)
    }

    pub(super) fn authored(
        &self,
        source: Option<&Authored>,
        role: SourceUseRole,
    ) -> Result<FieldOrigin> {
        match source {
            Some(Authored::Record(site)) => self.record_site(site, role),
            Some(Authored::RecordName(site)) => self.record_name_site(site, role),
            Some(Authored::Xml(site)) => self.xml_origin(&site.file, site.location.as_ref(), role),
            None => Ok(self.uses.origin(&[], false)),
        }
    }

    pub(super) fn reference_origin(
        &self,
        token: Option<&LoadoutReferenceSelection>,
        guid: Guid,
        role: SourceUseRole,
    ) -> Result<FieldOrigin> {
        let Some(site) = token
            .and_then(|token| self.facts.references.get(token))
            .filter(|site| matches!(site.datum, Datum::Reference { guid: original, .. } if original == guid))
        else {
            return Ok(self.uses.origin(&[], false));
        };
        self.record_site(site, role)
    }

    fn record_site(&self, site: &RecordSite, role: SourceUseRole) -> Result<FieldOrigin> {
        let stored = match &site.datum {
            Datum::Text(text) => {
                return Ok(text.origin(
                    &self.uses,
                    &self.forge_scope,
                    site.record,
                    site.ordinal,
                    role,
                    self.forge_complete,
                )?);
            }
            Datum::Reference { stored, .. } => *stored,
        };
        let location = SourceLocation::Subresource {
            namespace: "dataforgeValue".into(),
            indices: vec![u64::from(site.record), site.ordinal],
            field: "$value".into(),
        };
        let role = if stored {
            role
        } else {
            SourceUseRole::Selector
        };
        let original = self.forge_scope.at(location, role)?;
        Ok(self.uses.origin(&[original], self.forge_complete && stored))
    }

    fn record_name_site(&self, original: &RecordName, role: SourceUseRole) -> Result<FieldOrigin> {
        let location = SourceLocation::DataForge {
            record_index: original.record,
            field: "recordName".to_owned(),
        };
        let mut sites = vec![self.forge_scope.at(location, role)?];
        if let Some(raw) = &original.raw {
            sites.push(self.forge_scope.at(raw.clone(), role)?);
        }
        Ok(self
            .uses
            .origin(&sites, original.complete && self.forge_complete))
    }

    pub(super) fn xml_origin(
        &self,
        file: &LoadoutFileObservation,
        location: Option<&SourceLocation>,
        role: SourceUseRole,
    ) -> Result<FieldOrigin> {
        let Some(entry) = file.source().entry_origin() else {
            return Ok(self.uses.origin(&[], false));
        };
        let scope = self
            .uses
            .observe(SourceInput::Archive(entry), InputScope::Payload)?;
        let Some(location) = location else {
            let opaque = scope.at_opaque(role, SourceCoordinateOmission::Unavailable)?;
            return Ok(self.uses.origin(&[opaque], false));
        };
        let original = scope.at(location.clone(), role)?;
        Ok(self.uses.origin(
            &[original],
            entry.ancestors().all(|ancestor| ancestor.is_some()),
        ))
    }
}
