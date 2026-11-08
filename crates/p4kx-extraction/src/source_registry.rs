// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

use p4kx_archive::{ArchiveInputOrigin, EntryOrigin, EntryOriginStep};

use crate::DataForgeFileOrigin;
use crate::field_origin::FieldOrigin;
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

enum InputIdentity {
    Archive(ArchiveInputOrigin),
    File(DataForgeFileOrigin),
}

pub(crate) struct SourceRegistry {
    pub(crate) identity: Arc<()>,
    inputs: Vec<InputIdentity>,
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

    pub(crate) fn observe(&mut self, input: SourceInput<'_>) -> RegisteredSource {
        match input {
            SourceInput::Archive(origin) => self.archive(origin),
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

            SourceInput::File(origin) => {
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

    fn archive(&mut self, origin: &EntryOrigin) -> RegisteredSource {
        let input = self.archive_input(origin.input_origin());
        let nonce = if origin.ancestors().any(|step| step.is_none()) {
            format!("x{}", self.sources.len())
        } else {
            String::new()
        };
        let mut ancestors = Vec::new();
        let mut path = String::new();
        let mut effective = SourceEncryptionState::Unencrypted;
        let mut metadata = effective;
        let mut id = PhysicalSourceRef(String::new());
        let steps = origin
            .ancestors()
            .chain(std::iter::once(Some(origin.entry())))
            .enumerate();
        // Integer formatting into String cannot return a formatting error.
        for (position, step) in steps {
            metadata = effective;
            if let Some(step) = step {
                let _formatted = write!(path, ".{}", step.index());
                id = PhysicalSourceRef(format!("a{input}{nonce}{path}"));
                self.archive_entry(id.clone(), &ancestors, step, metadata);
                effective = effective.combine(SourceEncryptionState::of(step.encryption()));
            } else {
                let _formatted = write!(path, ".?{position}");
                id = PhysicalSourceRef(format!("a{input}{nonce}{path}"));
                self.unknown(id.clone(), ancestors.clone());
                effective = effective.combine(SourceEncryptionState::Unknown);
            }
            ancestors.push(id.clone());
        }
        RegisteredSource {
            id,
            payload: effective,
            metadata,
        }
    }

    fn archive_entry(
        &mut self,
        id: PhysicalSourceRef,
        ancestors: &[PhysicalSourceRef],
        step: &EntryOriginStep,
        metadata: SourceEncryptionState,
    ) {
        if !self.sources.contains_key(&id) {
            let source =
                source_registration::archive(self, id.clone(), ancestors.to_vec(), step, metadata);
            self.sources.insert(id, source);
        }
    }

    fn archive_input(&mut self, origin: &ArchiveInputOrigin) -> usize {
        if let Some(index) = self.inputs.iter().position(
            |known| matches!(known, InputIdentity::Archive(value) if value.is_same_input(origin)),
        ) {
            return index;
        }
        let index = self.inputs.len();
        self.inputs.push(InputIdentity::Archive(origin.clone()));
        index
    }

    fn file_input(&mut self, origin: &DataForgeFileOrigin) -> usize {
        if let Some(index) = self.inputs.iter().position(
            |known| matches!(known, InputIdentity::File(value) if value.is_same_input(origin)),
        ) {
            return index;
        }
        let index = self.inputs.len();
        self.inputs.push(InputIdentity::File(origin.clone()));
        index
    }

    fn unknown(&mut self, id: PhysicalSourceRef, ancestors: Vec<PhysicalSourceRef>) {
        self.sources.entry(id.clone()).or_insert(PhysicalSource {
            id,
            ancestors,
            stored_encryption: SourceEncryptionState::Unknown,
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
            })
        });
        SourceUse {
            description: Arc::clone(description),
            encryption,
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
