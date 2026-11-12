// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankDialogueEventRow, BankHircDialogueEvent, BankHircField, BankHircObject,
    BankNodePropertyKind,
};

use crate::audio_bank_dialogue_payload::{
    ARGUMENT, DialogueArgumentRecord, DialogueEventObservations, DialogueEventPayload,
    DialogueGroupType, SUMMARY,
};
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_node_property_rows::{self, PropertyRows, PropertyScope};
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

#[path = "audio_bank_dialogue_node.rs"]
mod node;
#[path = "audio_bank_dialogue_summary.rs"]
mod summary;

pub(crate) fn project<R: Read + Seek>(
    bank: &BankProjection<'_>,
    version: VersionSite,
    mut dialogue: BankHircDialogueEvent<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let original = dialogue.object();
    let body = original.body_offset()..original.body_end_exclusive();
    let sites = NodeSites::new(original, version, NodeKind::DialogueEvent);
    let mut origins = sites.record(bank.input, writer, None)?;
    let observed = summary::observed(&dialogue, &sites, &mut origins)?;
    let (declaration, status, issue) = if let Some(error) = dialogue.format_error() {
        let issue = MediaIssue::new(
            "audio.bank.invalid-dialogue-event",
            "HIRC",
            Severity::Error,
            error.to_string(),
        );
        (
            NodeDeclaration::Malformed {
                version: version.version,
                observed,
            },
            Status::Partial,
            Some(issue),
        )
    } else {
        (
            NodeDeclaration::Decoded {
                version: version.version,
                observed,
            },
            Status::Complete,
            None,
        )
    };
    sites.summary(&mut origins, Some(&body), issue.is_some())?;
    origins.finish().write(
        writer,
        &Subresource::new(SUMMARY, sites.indices().to_vec()),
        status,
        &DialogueEventPayload::Event(&declaration),
        issue.as_slice(),
    )?;

    original_rows(bank, &sites, &mut dialogue, writer)
}

fn original_rows<R: Read + Seek>(
    bank: &BankProjection<'_>,
    sites: &NodeSites,
    dialogue: &mut BankHircDialogueEvent<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for row in dialogue.rows() {
        match row.map_err(|cause| bank.replay_failure(cause))? {
            BankDialogueEventRow::Argument {
                ordinal,
                group_id,
                group_type_code,
            } => {
                argument(
                    bank,
                    sites,
                    ordinal,
                    &group_id,
                    group_type_code.as_ref(),
                    writer,
                )?;
            }
            BankDialogueEventRow::Node { ordinal, value } => {
                node::project(bank, sites, ordinal, &value, writer)?;
            }
            _ => return Err(MediaError::Contract("unexpected DialogueEvent row".into())),
        }
    }

    let projection = PropertyRows::new(bank, sites, PropertyScope::Node);
    for kind in [
        BankNodePropertyKind::Properties,
        BankNodePropertyKind::RangedModifiers,
    ] {
        if let Some(rows) = dialogue.properties(kind) {
            audio_bank_node_property_rows::emit(&projection, kind, rows, writer)?;
        }
    }
    Ok(())
}

pub(crate) fn unsupported(
    bank: &BankProjection<'_>,
    version: VersionSite,
    object: &BankHircObject,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = NodeSites::new(object, version, NodeKind::DialogueEvent);
    let mut origins = sites.record(bank.input, writer, None)?;
    sites.summary(&mut origins, None, false)?;
    let declaration = NodeDeclaration::<DialogueEventObservations>::Unsupported {
        version: version.version,
    };
    origins.finish().write(
        writer,
        &Subresource::new(SUMMARY, sites.indices().to_vec()),
        Status::Complete,
        &DialogueEventPayload::Event(&declaration),
        &[],
    )
}

fn argument(
    bank: &BankProjection<'_>,
    sites: &NodeSites,
    ordinal: u32,
    id: &BankHircField<u32>,
    original_type: Option<&BankHircField<u8>>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut origins = sites.record(bank.input, writer, Some(&id.range))?;
    sites.index(&mut origins, 2, &id.range, "dialogueArgumentPosition")?;
    sites.field(&mut origins, "group_id", &id.range, "dialogueGroupId")?;
    let group_type_code = sites.scalar(
        &mut origins,
        "group_type_code",
        original_type,
        "dialogueGroupTypeCode",
    )?;
    let group_type = if let Some(code) = original_type {
        sites.converted(
            &mut origins,
            "group_type",
            &code.range,
            "dialogueGroupTypeCode",
        )?;
        Some(DialogueGroupType::from(code.value))
    } else {
        None
    };
    let data = DialogueArgumentRecord {
        id: id.into(),
        type_code: group_type_code,
        kind: group_type,
    };
    let [section, object] = sites.indices();
    origins.finish().write(
        writer,
        &Subresource::new(ARGUMENT, vec![section, object, u64::from(ordinal)]),
        Status::Complete,
        &DialogueEventPayload::Argument(&data),
        &[],
    )
}
