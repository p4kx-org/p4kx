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
use super::record_origins::{Datum, RecordSite, Selection};

#[derive(Default)]
pub(super) struct Facts {
    pub(super) paths: HashMap<LoadoutSelection, Authored>,
    pub(super) classes: HashMap<LoadoutClassSelection, Authored>,
    pub(super) references: HashMap<LoadoutReferenceSelection, RecordSite>,
    pub(super) class_records: HashMap<LoadoutClassSelection, Option<u32>>,
    pub(super) definitions: HashMap<DefinitionKey, Definition>,
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
    Xml(Xml),
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
        self.authored(
            path.and_then(LoadoutPath::selection)
                .and_then(|selection| self.facts.paths.get(selection)),
            role,
        )
    }

    pub(super) fn authored(
        &self,
        source: Option<&Authored>,
        role: SourceUseRole,
    ) -> Result<FieldOrigin> {
        match source {
            Some(Authored::Record(site)) => self.record_site(site, role),
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
        let location = SourceLocation::Subresource {
            namespace: "dataforgeValue".into(),
            indices: vec![u64::from(site.record), site.ordinal],
            field: "$value".into(),
        };
        let (body_role, complete) = match &site.datum {
            Datum::Text(text) => (SourceUseRole::Selector, text.is_some()),
            Datum::Reference { stored: true, .. } => (role, true),
            Datum::Reference { .. } => (SourceUseRole::Selector, false),
        };
        let mut inputs = vec![self.forge_scope.at(location, body_role)?];
        if let Datum::Text(Some(location)) = &site.datum {
            inputs.push(self.forge_scope.at(location.clone(), role)?);
        }
        Ok(self.uses.origin(&inputs, self.forge_complete && complete))
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
