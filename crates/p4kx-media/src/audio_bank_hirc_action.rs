// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::ActionTarget;
use p4kx_extraction::bank::{
    BankActionFields, BankActionRead, BankHircObject, BankPlayBankDecision,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_projection::BankProjection;
use crate::hirc_object_index::HircObjectIndex;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

#[path = "audio_bank_hirc_action_origins.rs"]
mod origins;
#[path = "audio_bank_hirc_action_reference.rs"]
pub(crate) mod reference;

use self::reference::{ActionTargetCandidate, ActionTargetResolution};

pub(crate) const NAMESPACE: &str = "audio-bank-hirc-action";

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum ActionPayload {
    BankHircAction(ActionRecord),
    #[serde(rename = "bank-hirc-action-target-resolution")]
    TargetResolution(ActionTargetResolution),
    #[serde(rename = "bank-hirc-action-target-candidate")]
    TargetCandidate(ActionTargetCandidate),
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum ActionRecord {
    Decoded {
        action_type: u16,
        target_id: u32,
        play_bank: Option<PlayBankRecord>,
    },
    Malformed {
        action_type: Option<u16>,
        target_id: Option<u32>,
        play_bank: Option<PlayBankPrefix>,
    },
    Unsupported {
        action_type: Option<()>,
        target_id: Option<()>,
        play_bank: Option<()>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlayBankRecord {
    bank_id: u32,
    bank_type: u32,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlayBankPrefix {
    bank_id: Option<u32>,
    bank_type: Option<u32>,
}

impl ActionRecord {
    fn decoded(target: ActionTarget) -> Self {
        Self::Decoded {
            action_type: target.action_type,
            target_id: target.target_id,
            play_bank: target.bank.map(|bank| PlayBankRecord {
                bank_id: bank.bank_id,
                bank_type: bank.bank_type,
            }),
        }
    }

    fn malformed(fields: &BankActionFields) -> Self {
        let play_bank = match &fields.play_bank {
            BankPlayBankDecision::Play(fields) => Some(PlayBankPrefix {
                bank_id: fields.bank_id.as_ref().map(|field| field.value),
                bank_type: fields.bank_type.as_ref().map(|field| field.value),
            }),
            _ => None,
        };
        Self::Malformed {
            action_type: fields.action_type.as_ref().map(|field| field.value),
            target_id: fields.target_id.as_ref().map(|field| field.value),
            play_bank,
        }
    }

    const fn unsupported() -> Self {
        Self::Unsupported {
            action_type: None,
            target_id: None,
            play_bank: None,
        }
    }
}

pub(crate) fn project(
    projection: &BankProjection<'_>,
    object: &BankHircObject,
    version: VersionSite,
    action: BankActionRead,
    objects: &HircObjectIndex,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let (record, fields, status, issue, target) = match action {
        BankActionRead::NotAction => return Ok(()),
        BankActionRead::Decoded { target, fields } => (
            ActionRecord::decoded(target),
            Some(fields),
            Status::Complete,
            None,
            Some(target),
        ),
        BankActionRead::Malformed { error, fields } => (
            ActionRecord::malformed(&fields),
            Some(fields),
            Status::Partial,
            Some(MediaIssue::new(
                "audio.bank.invalid-action",
                "HIRC",
                Severity::Error,
                error.to_string(),
            )),
            None,
        ),
        BankActionRead::Unsupported { error } => (
            ActionRecord::unsupported(),
            None,
            Status::Complete,
            Some(MediaIssue::new(
                "audio.bank.unsupported-action",
                "HIRC",
                Severity::Info,
                error.to_string(),
            )),
            None,
        ),
        _ => {
            return Err(MediaError::Contract(
                "unexpected HIRC Action reader state".into(),
            ));
        }
    };
    let sites = origins::Sites::new(object, version, fields.as_ref());
    let prepared = origins::prepare(projection.input, &sites, issue.is_some(), writer)?;
    prepared.write(
        writer,
        &Subresource::new(NAMESPACE, vec![object.section_ordinal(), object.ordinal()]),
        status,
        &ActionPayload::BankHircAction(record),
        issue.as_slice(),
    )?;
    if let Some(target) = target {
        reference::project(projection, &sites, target, objects, writer)?;
    }
    Ok(())
}
