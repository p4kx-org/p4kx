// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use p4kx_archive::{ArchiveInputOrigin, ObservedEntryBinding};

use crate::field_origin::FieldOrigin;
use crate::source_archive_binding::ArchiveSourceBinding;
use crate::source_details::SourceDetails;
use crate::source_file_input::{FileInput, FileInputRef};
use crate::source_inventory::{
    PhysicalSource, PhysicalSourceKind, PhysicalSourceRef, SourceEncryptionState,
};
use crate::source_registration;
use crate::source_scope::SourceInput;
use crate::source_use::{
    InputScope, SourceLocation, SourceUse, SourceUseDescription, SourceUseRef, SourceUseRole,
};
use crate::source_value::{SourceValue, SourcedValue};

pub(crate) struct RegisteredSource {
    pub(crate) id: PhysicalSourceRef,
    pub(crate) payload: SourceEncryptionState,
    pub(crate) metadata: SourceEncryptionState,
}

pub(crate) enum InputIdentity {
    Archive {
        origin: ArchiveInputOrigin,
        source_refs: HashMap<ObservedEntryBinding, PhysicalSourceRef>,
    },
    ArchiveView(ArchiveSourceBinding),
    File(FileInput),
}

pub(crate) struct SourceRegistry {
    pub(crate) identity: Arc<()>,
    pub(crate) inputs: Vec<InputIdentity>,
    pub(crate) sources: BTreeMap<PhysicalSourceRef, PhysicalSource>,
    pub(crate) uses: BTreeMap<SourceUseRef, Arc<SourceUseDescription>>,
}

impl SourceRegistry {
    pub(crate) const fn new(identity: Arc<()>) -> Self {
        Self {
            identity,
            inputs: Vec::new(),
            sources: BTreeMap::new(),
            uses: BTreeMap::new(),
        }
    }

    pub(crate) fn observe(
        &mut self,
        input: SourceInput<'_>,
        details: SourceDetails,
    ) -> RegisteredSource {
        match input {
            SourceInput::Archive(origin) => self.archive(origin, details),
            SourceInput::ArchiveView(observed) => self.archive_view(observed, details),
            SourceInput::ArchiveFile { input, byte_length } => {
                let index = self.archive_input(input);
                let id = PhysicalSourceRef(format!("a{index}:file"));
                if !self.sources.contains_key(&id) {
                    let source = source_registration::archive_file(self, id.clone(), byte_length);
                    self.sources.insert(id.clone(), source);
                }
                RegisteredSource {
                    id,
                    payload: SourceEncryptionState::Unknown,
                    metadata: SourceEncryptionState::Unknown,
                }
            }

            SourceInput::File(origin) => self.file(FileInputRef::DataForge(origin)),
            SourceInput::BuildManifest(origin) => self.file(FileInputRef::BuildManifest(origin)),
            SourceInput::Unknown => {
                let id = PhysicalSourceRef(format!("u{}", self.sources.len()));
                self.unknown(id.clone(), Vec::new());
                RegisteredSource {
                    id,
                    payload: SourceEncryptionState::Unknown,
                    metadata: SourceEncryptionState::Unknown,
                }
            }
        }
    }

    pub(crate) fn archive_input(&mut self, origin: &ArchiveInputOrigin) -> usize {
        if let Some(index) = self.inputs.iter().position(
            |known| matches!(known, InputIdentity::Archive { origin: value, .. } if value.is_same_input(origin)),
        ) {
            return index;
        }
        let index = self.inputs.len();
        self.inputs.push(InputIdentity::Archive {
            origin: origin.clone(),
            source_refs: HashMap::new(),
        });
        index
    }

    fn file(&mut self, origin: FileInputRef<'_>) -> RegisteredSource {
        let index = self.file_input(origin);
        let id = PhysicalSourceRef(format!("f{index}"));
        if !self.sources.contains_key(&id) {
            let source = source_registration::file(self, id.clone(), origin);
            self.sources.insert(id.clone(), source);
        }
        RegisteredSource {
            id,
            payload: SourceEncryptionState::Unknown,
            metadata: SourceEncryptionState::Unknown,
        }
    }

    fn file_input(&mut self, origin: FileInputRef<'_>) -> usize {
        if let Some(index) = self.inputs.iter().position(
            |known| matches!(known, InputIdentity::File(value) if origin.is_same_input(value)),
        ) {
            return index;
        }
        let index = self.inputs.len();
        self.inputs.push(InputIdentity::File(origin.into_owned()));
        index
    }

    pub(crate) fn unknown(&mut self, id: PhysicalSourceRef, ancestors: Vec<PhysicalSourceRef>) {
        self.sources.entry(id.clone()).or_insert(PhysicalSource {
            id,
            ancestors,
            stored_encryption: SourceEncryptionState::Unknown,
            binding: None,
            source: PhysicalSourceKind::Unknown,
        });
    }

    pub(crate) fn register_use(
        &mut self,
        source: &PhysicalSourceRef,
        scope: InputScope,
        location: SourceLocation,
        role: SourceUseRole,
        encryption: SourceEncryptionState,
    ) -> SourceUse {
        let id = SourceUseRef(format!(
            "u{}:{}:{}:{}:{}",
            source.0.len(),
            source.0,
            scope.key(),
            role.key(),
            location.key()
        ));
        let description = self.uses.entry(id.clone()).or_insert_with(|| {
            Arc::new(SourceUseDescription {
                id: id.clone(),
                source: source.clone(),
                scope,
                role,
                location,
                effective_encryption: encryption,
            })
        });
        SourceUse {
            description: Arc::clone(description),
            identity: Arc::clone(&self.identity),
        }
    }

    pub(crate) fn file_metadata_value<T>(
        &mut self,
        source: &PhysicalSourceRef,
        field: String,
        value: SourceValue<T>,
    ) -> SourcedValue<T> {
        let used = self.register_use(
            source,
            InputScope::FileMetadata,
            SourceLocation::Metadata { field },
            SourceUseRole::Payload,
            SourceEncryptionState::Unknown,
        );
        SourcedValue::new(value, FieldOrigin::new([&used], true))
    }

    pub(crate) fn metadata_value<T>(
        &mut self,
        source: &PhysicalSourceRef,
        field: String,
        value: SourceValue<T>,
        encryption: SourceEncryptionState,
    ) -> SourcedValue<T> {
        let used = self.register_use(
            source,
            InputScope::ArchiveDirectoryMetadata,
            SourceLocation::Metadata { field },
            SourceUseRole::Payload,
            encryption,
        );
        SourcedValue::new(value, FieldOrigin::new([&used], true))
    }
}
