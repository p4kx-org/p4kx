// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::CafImageWord;
use p4kx_extraction::{FieldPath, SourceUseRole};

use super::super::origin as proof;
use super::ImageSource;
use crate::directory_lookup::animation_banks::AnimationBanks;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_reference::{ReferenceIdentifier, ReferenceOriginInputs, ReferenceSubresource};
use crate::{IntegerWidth, MediaError, MediaWriter, OriginalIdentifier, Reference, origin_error};

const FIELD: &str = "/data/payload/dba_entry";

pub(super) struct ObservedBank {
    pub(super) reference: Box<Reference>,
    pub(super) position: Option<usize>,
    pub(super) selection: crate::origin_contributions::OriginContributions,
}

impl ImageSource<'_, '_> {
    pub(super) fn dba_entry(
        &self,
        hash: CafImageWord,
        prepared: &mut PreparedRecordOrigins,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Option<ObservedBank>, MediaError> {
        let selector =
            self.origins
                .payload(hash.range(), "dbaPathHash", SourceUseRole::Payload, writer)?;
        if hash.value == 0 {
            proof::at(prepared, FIELD, selector, writer)?;
            return Ok(None);
        }
        let directory = self.context.directory;
        if self.banks.get().is_none() {
            let banks = AnimationBanks::read(directory, writer)?;
            self.banks.set(banks).map_err(|_| {
                MediaError::Contract("DBA path index is already initialized".into())
            })?;
        }
        let banks = self
            .banks
            .get()
            .ok_or_else(|| MediaError::Contract("DBA path index was not initialized".into()))?;
        let original = OriginalIdentifier::try_new_with_unsigned(
            "animation.dba-path-crc32",
            IntegerWidth::Bits32,
            u64::from(hash.value),
        )?;
        let observed = banks.lookup(directory, hash.value, original, writer)?;
        let address = FieldPath::parse(FIELD).map_err(|cause| origin_error::address(&cause))?;
        let proof = ReferenceOriginInputs {
            original: &selector,
            selector: &selector,
            inventory: &observed.inventory,
            semantic: &selector,
            candidates: &observed.evidence,
            read_selected: false,
            identifier: ReferenceIdentifier::Unsigned,
            subresource: ReferenceSubresource::None,
        };
        let selection = proof.resolution(writer)?;
        proof.write(prepared, &address, &observed.lookup.reference, writer)?;
        Ok(Some(ObservedBank {
            reference: Box::new(observed.lookup.reference),
            position: observed.lookup.input.map(|input| input.position),
            selection,
        }))
    }
}
