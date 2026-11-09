// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::sync::Arc;

use p4kx_extraction::{
    InputScope, SourceCoordinateOmission, SourceEncryptionState, SourceLocation, SourceUse,
    SourceUseRole, SourceUses,
};

use crate::origin_contributions::{OriginContributions, OriginCut};
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaWriter, origin_error};

/// A borrowed original source coordinate, independent of the output JSON address.
#[derive(Clone, Copy)]
pub(crate) struct OriginSite<'a> {
    namespace: &'a str,
    indices: &'a [u64],
    field: &'a str,
}

impl<'a> OriginSite<'a> {
    pub(crate) const fn new(namespace: &'a str, indices: &'a [u64], field: &'a str) -> Self {
        Self {
            namespace,
            indices,
            field,
        }
    }

    fn fits(&self, limit: u64) -> bool {
        let size = self
            .namespace
            .len()
            .checked_add(self.field.len())
            .and_then(|size| {
                self.indices
                    .len()
                    .checked_mul(size_of::<u64>())
                    .and_then(|indices| size.checked_add(indices))
            });
        !self.namespace.is_empty()
            && size
                .and_then(|size| u64::try_from(size).ok())
                .is_some_and(|size| size <= limit)
    }
}

/// Per-datum optional capture whose cuts cannot starve later independent fields.
pub(crate) struct OriginCapture {
    owner: SourceUses,
    uses: BTreeMap<String, SourceUse>,
    witness: Option<SourceUse>,
    complete: bool,
    cut: bool,
    bytes: u64,
    cuts: Vec<OriginCut>,
    limit: u64,
}

impl OriginCapture {
    pub(crate) fn with_limit(owner: &SourceUses, limit: u64) -> Self {
        Self {
            owner: owner.clone(),
            uses: BTreeMap::new(),
            witness: None,
            complete: true,
            cut: false,
            bytes: 0,
            cuts: Vec::new(),
            limit,
        }
    }

    pub(crate) fn add(
        &mut self,
        input: &MediaOriginInput,
        scope: InputScope,
        site: OriginSite<'_>,
        role: SourceUseRole,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        self.require_owner(&input.owner, writer)?;
        self.add_original(input, scope, site, role)
    }

    pub(crate) fn add_original(
        &mut self,
        input: &MediaOriginInput,
        scope: InputScope,
        site: OriginSite<'_>,
        role: SourceUseRole,
    ) -> Result<(), MediaError> {
        if !self.owner.is_same_inventory(&input.owner) {
            return Err(origin_error::foreign());
        }
        let contributes = matches!(
            role,
            SourceUseRole::Payload | SourceUseRole::Selector | SourceUseRole::Conversion
        );
        if input.details_limited && contributes {
            self.cut(OriginCut::InputMetadata);
        }
        let original = input.scope.for_scope(scope);
        let actual = if site.fits(self.limit) {
            original.at(
                SourceLocation::Subresource {
                    namespace: site.namespace.to_owned(),
                    indices: site.indices.to_vec(),
                    field: site.field.to_owned(),
                },
                role,
            )
        } else {
            if contributes {
                self.complete = false;
                self.cut(OriginCut::UseMetadata);
            }
            original.at_opaque(role, SourceCoordinateOmission::CaptureLimit)
        }
        .map_err(origin_error::evidence)?;
        self.retain(actual, self.limit)
    }
    pub(crate) fn include(
        &mut self,
        contribution: &OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        self.require_owner(&contribution.owner, writer)?;
        self.include_original(contribution)
    }

    pub(crate) fn include_original(
        &mut self,
        contribution: &OriginContributions,
    ) -> Result<(), MediaError> {
        if !self.owner.is_same_inventory(&contribution.owner) {
            return Err(origin_error::foreign());
        }
        self.complete &= contribution.complete;
        for cut in contribution.cuts.iter().copied() {
            self.cut(cut);
        }
        for actual in contribution.uses.iter().cloned() {
            self.retain(actual, self.limit)?;
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> OriginContributions {
        let uses: Arc<[SourceUse]> = if self.cut {
            self.witness.into_iter().collect::<Vec<_>>().into()
        } else {
            self.uses.into_values().collect::<Vec<_>>().into()
        };
        OriginContributions {
            owner: self.owner,
            uses,
            complete: self.complete,
            cuts: self.cuts.into(),
        }
    }

    fn retain(&mut self, actual: SourceUse, limit: u64) -> Result<(), MediaError> {
        if !matches!(
            actual.role(),
            SourceUseRole::Payload | SourceUseRole::Selector | SourceUseRole::Conversion
        ) {
            return Ok(());
        }
        if self.cut {
            return self.retain_witness(&actual);
        }
        let key = actual.reference().as_str();
        if self.uses.contains_key(key) {
            return Ok(());
        }
        let bytes = u64::try_from(key.len())
            .ok()
            .and_then(|bytes| bytes.checked_add(size_of::<SourceUse>() as u64))
            .and_then(|bytes| self.bytes.checked_add(bytes));
        if let Some(bytes) = bytes.filter(|bytes| *bytes <= limit) {
            self.bytes = bytes;
            self.uses.insert(key.to_owned(), actual);
            return Ok(());
        }
        self.complete = false;
        self.cut = true;
        self.cut(OriginCut::FieldUses);
        let retained = std::mem::take(&mut self.uses);
        for original in retained.into_values() {
            self.retain_witness(&original)?;
        }
        self.retain_witness(&actual)
    }

    fn retain_witness(&mut self, actual: &SourceUse) -> Result<(), MediaError> {
        if self
            .witness
            .as_ref()
            .is_none_or(|previous| witness_rank(actual) > witness_rank(previous))
        {
            self.witness = Some(
                self.owner
                    .opaque_use(actual, SourceCoordinateOmission::CaptureLimit)
                    .map_err(origin_error::evidence)?,
            );
        }
        Ok(())
    }

    fn require_owner(
        &self,
        owner: &SourceUses,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        writer.checkpoint()?;
        if !self.owner.is_same_inventory(owner)
            || !self.owner.is_same_inventory(&writer.origins.owner)
        {
            return Err(origin_error::foreign());
        }
        Ok(())
    }

    fn cut(&mut self, cut: OriginCut) {
        if !self.cuts.contains(&cut) {
            self.cuts.push(cut);
        }
    }
}

fn witness_rank(actual: &SourceUse) -> u8 {
    match actual.encryption() {
        SourceEncryptionState::Encrypted => 2,
        SourceEncryptionState::Unknown => 1,
        _ => 0,
    }
}
