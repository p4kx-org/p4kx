// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};

use p4kx_audio::{
    BankHircContainerNode, BankHircField, BankSwitchConfiguration, BankSwitchGroup,
    BankSwitchMember, BankSwitchMembership,
};
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::source_evidence_error::SourceEvidenceError;

use super::super::origins::ObjectOrigins;
use super::dto::{SwitchFade, SwitchReferenceState, SwitchSourceRow};

pub(super) struct SwitchRows<'view, 'input, R> {
    container: RefCell<BankHircContainerNode<'input, R>>,
    origins: &'view ObjectOrigins,
}

impl<'view, 'input, R> SwitchRows<'view, 'input, R> {
    pub(super) const fn new(
        container: BankHircContainerNode<'input, R>,
        origins: &'view ObjectOrigins,
    ) -> Self {
        Self {
            container: RefCell::new(container),
            origins,
        }
    }
}

impl<R: Read + Seek> Serialize for SwitchRows<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut container = self.container.try_borrow_mut().map_err(S::Error::custom)?;
        let mut sequence = serializer.serialize_seq(None)?;
        if let Some(rows) = container.switch_groups() {
            for original in rows {
                let row = match original.map_err(S::Error::custom)? {
                    BankSwitchMembership::Group(original) => group(&original, self.origins),
                    BankSwitchMembership::Member(original) => member(&original, self.origins),
                    _ => return Err(S::Error::custom("unexpected Switch membership row")),
                }
                .map_err(S::Error::custom)?;
                sequence.serialize_element(&row)?;
            }
        }
        if let Some(rows) = container.switch_configurations() {
            for original in rows {
                let original = original.map_err(S::Error::custom)?;
                let row = configuration(&original, self.origins).map_err(S::Error::custom)?;
                sequence.serialize_element(&row)?;
            }
        }
        sequence.end()
    }
}

fn group(
    original: &BankSwitchGroup,
    origins: &ObjectOrigins,
) -> Result<SwitchSourceRow, SourceEvidenceError> {
    let members_range = original
        .members_range
        .as_ref()
        .map(|range| origins.range(range, "switchMembershipMembersExtent"))
        .transpose()?;
    Ok(SwitchSourceRow::Group {
        ordinal: origins.derived(
            original.ordinal,
            &original.value_id.range,
            "switchMembershipGroupPosition",
        )?,
        range: origins.range(&original.checked_range, "switchMembershipGroupExtent")?,
        value_id: origins.field(&original.value_id, "switchMembershipValueId")?,
        member_count: origins.optional(original.member_count.as_ref(), "switchMembershipCount")?,
        members_range,
        complete: origins.derived(
            original.complete,
            &original.checked_range,
            "switchMembershipGroupCoverage",
        )?,
    })
}

fn member(
    original: &BankSwitchMember,
    origins: &ObjectOrigins,
) -> Result<SwitchSourceRow, SourceEvidenceError> {
    let field = &original.child_id;
    Ok(SwitchSourceRow::Member {
        group_ordinal: origins.derived(
            original.group_ordinal,
            &original.group_value_range,
            "switchMembershipGroupPosition",
        )?,
        ordinal: origins.derived(original.ordinal, &field.range, "switchMembershipPosition")?,
        child_id: origins.field(field, "switchMembershipChildId")?,
        reference_resolution: origins.derived(
            SwitchReferenceState::Unchecked,
            &field.range,
            "switchMembershipChildId",
        )?,
    })
}

fn configuration(
    original: &BankSwitchConfiguration,
    origins: &ObjectOrigins,
) -> Result<SwitchSourceRow, SourceEvidenceError> {
    let child = &original.child_id;
    Ok(SwitchSourceRow::Configuration {
        ordinal: origins.derived(
            original.ordinal,
            &child.range,
            "switchConfigurationPosition",
        )?,
        range: origins.range(&original.checked_range, "switchConfigurationExtent")?,
        child_id: origins.field(child, "switchConfigurationChildId")?,
        flags: origins.optional(original.flags.as_ref(), "switchConfigurationFlags")?,
        fade_out: fade(
            original.fade_out_bits.as_ref(),
            origins,
            "switchFadeOutBits",
            "switchSignedFadeOut",
        )?,
        fade_in: fade(
            original.fade_in_bits.as_ref(),
            origins,
            "switchFadeInBits",
            "switchSignedFadeIn",
        )?,
    })
}

fn fade(
    field: Option<&BankHircField<u32>>,
    origins: &ObjectOrigins,
    bits: &'static str,
    signed: &'static str,
) -> Result<Option<SwitchFade>, SourceEvidenceError> {
    let Some(field) = field else {
        return Ok(None);
    };
    let value = i32::from_le_bytes(field.value.to_le_bytes());
    Ok(Some(SwitchFade {
        bits: origins.field(field, bits)?,
        signed_value: origins.derived(value, &field.range, signed)?,
    }))
}
