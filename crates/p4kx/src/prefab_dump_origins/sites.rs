// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use anyhow::{Context, Result};
use p4kx_assembly::{
    PrefabFile, PrefabLocation, PrefabSet, PrefabUnreadableEvidence, PrefabUnreadableSource,
};
use p4kx_extraction::cryxml::{SourceAttributePosition, SourceElementPosition};
use p4kx_extraction::model::PrefabDiagnostic;
use p4kx_extraction::{
    Archive, Entry, FieldOrigin, FieldOrigins, FieldPath, InputScope, SharedFieldOrigin,
    SourceCoordinateOmission, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};

use super::origins;

#[derive(Clone)]
pub(super) struct BoundScope {
    pub(super) scope: SourceScope,
    pub(super) complete: bool,
}

#[derive(Clone)]
pub(super) struct Proof {
    pub(super) uses: Vec<SourceUse>,
    pub(super) complete: bool,
}

impl Default for Proof {
    fn default() -> Self {
        Self {
            uses: Vec::new(),
            complete: true,
        }
    }
}

impl Proof {
    pub(super) fn capture(
        &mut self,
        scope: &BoundScope,
        location: Option<SourceLocation>,
        role: SourceUseRole,
    ) -> Result<()> {
        self.complete &= scope.complete;
        let origin = match location {
            Some(location) => scope.scope.at(location, role)?,
            None => scope
                .scope
                .at_opaque(role, SourceCoordinateOmission::Unavailable)?,
        };
        self.uses.push(origin);
        Ok(())
    }

    pub(super) fn append(&mut self, other: Self) {
        self.complete &= other.complete;
        self.uses.extend(other.uses);
    }
}

#[derive(Clone, Copy)]
pub(super) enum PrefabAttribute {
    Name,
    Id,
}

pub(super) struct Sites<'set, 'archive> {
    archive: &'archive Archive,
    set: &'set PrefabSet<'archive>,
    scopes: HashMap<usize, BoundScope>,
    pub(super) uses: SourceUses,
}

impl<'set, 'archive> Sites<'set, 'archive> {
    pub(super) fn new(archive: &'archive Archive, set: &'set PrefabSet<'archive>) -> Self {
        Self {
            archive,
            set,
            scopes: HashMap::new(),
            uses: SourceUses::with_compact_use_refs(),
        }
    }

    fn scope_for_entry(&mut self, entry: &'archive Entry) -> Result<&BoundScope> {
        use std::collections::hash_map::Entry;

        match self.scopes.entry(entry.index()) {
            Entry::Occupied(known) => Ok(known.into_mut()),
            Entry::Vacant(vacant) => {
                let (scope, complete) = origins::input(self.archive, entry, &self.uses)?;
                Ok(vacant.insert(BoundScope { scope, complete }))
            }
        }
    }

    pub(super) fn scope_for_file(&mut self, file: &PrefabFile) -> Result<&BoundScope> {
        let entry = self
            .set
            .source_entry(file)
            .context("physical prefab library")?;
        self.scope_for_entry(entry)
    }

    fn directory_entry(&mut self, index: usize) -> Result<&BoundScope> {
        let entry = self
            .archive
            .entries()
            .get(index)
            .context("prefab lookup Entry")?;
        self.scope_for_entry(entry)
    }

    pub(super) fn failed_lookup(&mut self, index: usize) -> Result<()> {
        let _attempt = self.directory_entry(index)?.scope.at_opaque(
            SourceUseRole::Attempted,
            SourceCoordinateOmission::Unavailable,
        )?;
        Ok(())
    }

    pub(super) fn unreadable(
        &mut self,
        proof: &mut Proof,
        evidence: &PrefabUnreadableEvidence,
    ) -> Result<()> {
        let scope = self.directory_entry(evidence.entry_index)?;
        Self::directory_field(proof, scope, "path", SourceUseRole::Conversion)?;
        match evidence.source.as_ref() {
            None => {}
            Some(PrefabUnreadableSource::EntrySize) => {
                Self::directory_field(
                    proof,
                    scope,
                    "uncompressed_size",
                    SourceUseRole::Conversion,
                )?;
            }
            Some(PrefabUnreadableSource::Payload(PrefabDiagnostic::RootName(position))) => {
                let location = position
                    .as_ref()
                    .and_then(|at| SourceLocation::xml_element(at, "name"));
                proof.capture(scope, location, SourceUseRole::Payload)?;
            }
            Some(PrefabUnreadableSource::Payload(_)) => {
                proof.capture(scope, None, SourceUseRole::Conversion)?;
            }
            _ => proof.complete = false,
        }
        Ok(())
    }

    pub(super) fn file_selector(&mut self, proof: &mut Proof, file: &PrefabFile) -> Result<()> {
        Self::directory_field(
            proof,
            self.scope_for_file(file)?,
            "path",
            SourceUseRole::Selector,
        )
    }

    fn directory_field(
        proof: &mut Proof,
        scope: &BoundScope,
        field: &str,
        role: SourceUseRole,
    ) -> Result<()> {
        proof.complete &= scope.complete;
        proof.uses.push(
            scope
                .scope
                .for_scope(InputScope::ArchiveDirectoryMetadata)
                .at(
                    SourceLocation::Metadata {
                        field: field.to_owned(),
                    },
                    role,
                )?,
        );
        Ok(())
    }

    pub(super) fn survey_directory(&self, proof: &mut Proof) -> Result<()> {
        let observation = self.uses.observe_root_directory_paths(self.archive)?;
        proof.complete &= observation.is_some();
        proof.uses.extend(observation);
        Ok(())
    }

    pub(super) fn element(
        &mut self,
        proof: &mut Proof,
        file: &PrefabFile,
        node: Option<&SourceElementPosition>,
        name: &str,
        role: SourceUseRole,
    ) -> Result<()> {
        proof.capture(
            self.scope_for_file(file)?,
            node.and_then(|at| SourceLocation::xml_element(at, name)),
            role,
        )
    }

    pub(super) fn attribute(
        &mut self,
        proof: &mut Proof,
        file: &PrefabFile,
        node: Option<&SourceElementPosition>,
        attribute: Option<&SourceAttributePosition>,
        name: &str,
        role: SourceUseRole,
    ) -> Result<()> {
        if let Some(at) = attribute {
            proof.capture(
                self.scope_for_file(file)?,
                SourceLocation::xml_attribute(at, name),
                role,
            )
        } else {
            let role = if role == SourceUseRole::Payload {
                SourceUseRole::Conversion
            } else {
                role
            };
            self.element(proof, file, node, name, role)
        }
    }

    pub(super) fn prefab_attribute(
        &mut self,
        proof: &mut Proof,
        prefab: PrefabLocation<'_>,
        field: PrefabAttribute,
        role: SourceUseRole,
    ) -> Result<()> {
        let row = prefab.source();
        let (name, original) = match field {
            PrefabAttribute::Name => ("Name", row.and_then(|row| row.name.as_ref())),
            PrefabAttribute::Id => ("Id", row.and_then(|row| row.id.as_ref())),
        };
        self.attribute(
            proof,
            prefab.file,
            row.map(|row| &row.element),
            original,
            name,
            role,
        )
    }

    pub(super) fn objects(
        &mut self,
        proof: &mut Proof,
        prefab: PrefabLocation<'_>,
        role: SourceUseRole,
    ) -> Result<()> {
        let scope = prefab
            .source()
            .map(|row| row.objects.as_ref().unwrap_or(&row.element));
        self.element(proof, prefab.file, scope, "Objects", role)
    }

    pub(super) fn put_origin(
        fields: &mut FieldOrigins,
        path: &str,
        origin: FieldOrigin,
    ) -> Result<()> {
        fields.insert(FieldPath::parse(path)?, origin)?;
        Ok(())
    }

    pub(super) fn put(&self, fields: &mut FieldOrigins, path: &str, proof: &Proof) -> Result<()> {
        Self::put_origin(fields, path, self.uses.origin(&proof.uses, proof.complete))
    }

    pub(super) fn put_shared(
        &self,
        fields: &mut FieldOrigins,
        path: &str,
        shared: &SharedFieldOrigin,
        proof: &Proof,
    ) -> Result<()> {
        Self::put_origin(fields, path, self.shared_origin(shared, proof)?)
    }

    pub(super) fn shared_origin(
        &self,
        shared: &SharedFieldOrigin,
        proof: &Proof,
    ) -> Result<FieldOrigin> {
        Ok(self
            .uses
            .origin_with_shared(shared, &proof.uses, proof.complete)?)
    }
}
