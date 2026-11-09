// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::origin_contract::OriginProfile;
use crate::origin_resolver::BuiltinOrigins;

use p4kx_extraction::{FieldPath, SourceUses};
use serde::Serialize;

use crate::origin_addresses::OriginAddresses;
use crate::origin_capture::OriginCapture;
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaIssue, MediaWriter, Status, Subresource, origin_error};

/// Exact per-datum prepared origins retaining their original shared inventory owner.
#[derive(Debug)]
pub(crate) struct PreparedRecordOrigins {
    owner: SourceUses,
    addresses: OriginAddresses,
    builtin: Option<BuiltinOrigins>,
    profile: Option<OriginProfile>,
}

impl PreparedRecordOrigins {
    pub(crate) fn new(owner: &SourceUses) -> Self {
        Self {
            owner: owner.clone(),
            addresses: OriginAddresses::default(),
            builtin: None,
            profile: None,
        }
    }

    /// Consumes this occurrence's prepared origins while writing its actual record.
    pub(crate) fn write<T: Serialize>(
        self,
        writer: &mut MediaWriter<'_>,
        subresource: &Subresource,
        status: Status,
        data: &T,
        issues: &[MediaIssue],
    ) -> Result<(), MediaError> {
        self.require_writer(writer)?;
        writer.write_record(
            subresource,
            status,
            data,
            issues,
            self.builtin(),
            Some(&self),
        )
    }

    pub(crate) fn insert(
        &mut self,
        path: &FieldPath,
        contributions: OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        self.require_writer(writer)?;
        if !self.owner.is_same_inventory(&contributions.owner) {
            return Err(origin_error::foreign());
        }
        self.addresses.insert(path, contributions)
    }

    /// Shares one actual typed source value's contributions across its serialized components.
    pub(crate) fn insert_subtree(
        &mut self,
        path: &FieldPath,
        contributions: OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        self.require_writer(writer)?;
        if !self.owner.is_same_inventory(&contributions.owner) {
            return Err(origin_error::foreign());
        }
        self.addresses.insert_subtree(path, contributions)
    }

    pub(crate) const fn set_profile(&mut self, profile: OriginProfile) {
        self.profile = Some(profile);
    }

    pub(crate) const fn profile(&self) -> OriginProfile {
        match &self.builtin {
            Some(builtin) => builtin.profile,
            None => match self.profile {
                Some(profile) => profile,
                None => OriginProfile::Unmigrated,
            },
        }
    }

    pub(crate) fn set_builtin(&mut self, builtin: BuiltinOrigins) -> Result<(), MediaError> {
        if !self.owner.is_same_inventory(&builtin.input.owner) {
            return Err(origin_error::foreign());
        }
        self.builtin = Some(builtin);
        Ok(())
    }

    pub(crate) const fn builtin(&self) -> Option<&BuiltinOrigins> {
        self.builtin.as_ref()
    }

    pub(crate) fn visit(&self, path: &str) -> Option<&OriginContributions> {
        self.addresses.visit(path)
    }

    pub(crate) fn has_known_descendants(&self, prefix: &str) -> bool {
        self.addresses.has_known_descendants(prefix)
    }
    pub(crate) fn include_known(
        &self,
        prefix: &str,
        capture: &mut OriginCapture,
    ) -> Result<(), MediaError> {
        self.addresses.include_known(prefix, capture)
    }
    pub(crate) fn require_consumed(&self) -> Result<(), MediaError> {
        self.addresses.require_consumed()
    }

    pub(crate) fn require_writer(&self, writer: &MediaWriter<'_>) -> Result<(), MediaError> {
        writer.checkpoint()?;
        if !self.owner.is_same_inventory(&writer.origins.owner) {
            return Err(origin_error::foreign());
        }
        Ok(())
    }
}
