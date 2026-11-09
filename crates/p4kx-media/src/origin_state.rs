// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::OnceLock;

use p4kx_extraction::{Archive, ArchiveInputOrigin, Entry, InputScope, SourceInput, SourceUses};

use crate::origin_input::MediaOriginInput;
use crate::{MediaError, origin_error};

/// One original evidence authority and the actual open-container input stack.
pub(crate) struct OriginState {
    pub(crate) owner: SourceUses,
    pub(crate) root: Option<MediaOriginInput>,
    directories: Vec<MediaOriginInput>,
    detail_bytes: u64,
    root_input: OnceLock<ArchiveInputOrigin>,
}

impl OriginState {
    pub(crate) fn new(detail_bytes: u64) -> Self {
        Self {
            owner: SourceUses::new(),
            root: None,
            directories: Vec::new(),
            detail_bytes,
            root_input: OnceLock::new(),
        }
    }

    pub(crate) fn observe_root(
        &mut self,
        archive: &Archive,
        byte_length: u64,
    ) -> Result<(), MediaError> {
        self.require_root(archive)?;
        let scope = self
            .owner
            .observe(
                SourceInput::ArchiveFile {
                    input: archive.input_origin(),
                    byte_length,
                },
                InputScope::Payload,
            )
            .map_err(origin_error::evidence)?;
        let input = MediaOriginInput {
            owner: self.owner.clone(),
            scope,
            details_limited: false,
        };
        self.root = Some(input.clone());
        self.directories.push(input);
        Ok(())
    }

    pub(crate) fn input(
        &self,
        archive: &Archive,
        entry: &Entry,
    ) -> Result<MediaOriginInput, MediaError> {
        self.require_root(archive)?;
        MediaOriginInput::observe(&self.owner, archive, entry, self.detail_bytes)
    }

    pub(crate) fn directory(&self) -> Option<&MediaOriginInput> {
        self.directories.last()
    }

    pub(crate) fn push_directory(&mut self, input: MediaOriginInput) -> Result<(), MediaError> {
        if !self.owner.is_same_inventory(&input.owner) {
            return Err(origin_error::foreign());
        }
        self.directories.push(input);
        Ok(())
    }

    pub(crate) fn pop_directory(&mut self) -> Result<(), MediaError> {
        if self.directories.len() <= 1 {
            return Err(MediaError::Contract(
                "media origin container stack underflow".into(),
            ));
        }
        self.directories.pop();
        Ok(())
    }
    fn require_root(&self, archive: &Archive) -> Result<(), MediaError> {
        let actual = self
            .root_input
            .get_or_init(|| archive.input_origin().clone());
        if !actual.is_same_input(archive.input_origin()) {
            return Err(origin_error::foreign());
        }
        Ok(())
    }
}
