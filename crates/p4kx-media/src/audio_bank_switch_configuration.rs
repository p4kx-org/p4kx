// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{BankHircField, BankSwitchConfiguration, BankSwitchConfigurationRows};

use crate::audio_bank_node_declaration_payload::NodeReferenceStatus;
use crate::audio_bank_node_declarations::Declarations;
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_switch_container_payload::{
    CONFIG_NAMESPACE, SwitchConfigurationRecord, SwitchContainerPayload, SwitchFadeWord,
};
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

pub(crate) fn project<R: Read + Seek>(
    declarations: &Declarations<'_, '_>,
    rows: BankSwitchConfigurationRows<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = declarations.sites(NodeKind::SwitchContainer);
    for row in rows {
        let row = row.map_err(|cause| declarations.bank.replay_failure(cause))?;
        emit(declarations, &sites, &row, writer)?;
    }
    Ok(())
}

fn emit(
    declarations: &Declarations<'_, '_>,
    sites: &NodeSites,
    row: &BankSwitchConfiguration,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = sites.record(declarations.bank.input, writer, Some(&row.child_id.range))?;
    sites.converted(
        &mut prepared,
        "layout",
        &row.checked_range,
        "switchConfigurationCoverage",
    )?;
    let record = read(sites, &mut prepared, row)?;
    let (status, issue) = if matches!(record, SwitchConfigurationRecord::Malformed { .. }) {
        sites.issues(&mut prepared, &row.checked_range)?;
        (
            Status::Partial,
            Some(MediaIssue::new(
                "audio.bank.truncated-switch-configuration",
                "HIRC",
                Severity::Error,
                "the original child configuration ends before all its fields fit",
            )),
        )
    } else {
        (Status::Complete, None)
    };
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(
            CONFIG_NAMESPACE,
            vec![section, object, u64::from(row.ordinal)],
        ),
        status,
        &SwitchContainerPayload::Configuration(&record),
        issue.as_slice(),
    )
}

fn read(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    row: &BankSwitchConfiguration,
) -> Result<SwitchConfigurationRecord, MediaError> {
    sites.field(
        prepared,
        "child_id",
        &row.child_id.range,
        "switchConfigurationChildId",
    )?;
    sites.converted(
        prepared,
        "reference_resolution",
        &row.child_id.range,
        "switchReferenceUnchecked",
    )?;
    let child_id = (&row.child_id).into();
    let reference_resolution = NodeReferenceStatus::Unchecked;
    let flags = sites.scalar(
        prepared,
        "flags",
        row.flags.as_ref(),
        "switchConfigurationFlags",
    )?;
    let fade_out = fade(sites, prepared, row.fade_out_bits.as_ref(), Fade::Out)?;
    let fade_in = fade(sites, prepared, row.fade_in_bits.as_ref(), Fade::In)?;
    match (flags, fade_out, fade_in) {
        (Some(flags), Some(fade_out), Some(fade_in)) => Ok(SwitchConfigurationRecord::Decoded {
            child_id,
            flags,
            fade_out,
            fade_in,
            reference_resolution,
        }),
        (flags, fade_out, fade_in) => Ok(SwitchConfigurationRecord::Malformed {
            child_id,
            flags,
            fade_out,
            fade_in,
            reference_resolution,
        }),
    }
}

#[derive(Clone, Copy)]
enum Fade {
    Out,
    In,
}

fn fade(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    word: Option<&BankHircField<u32>>,
    direction: Fade,
) -> Result<Option<SwitchFadeWord>, MediaError> {
    let Some(word) = word else { return Ok(None) };
    let (name, payload_label, conversion_label) = match direction {
        Fade::Out => ("fade_out", "switchFadeOutBits", "switchSignedFadeOut"),
        Fade::In => ("fade_in", "switchFadeInBits", "switchSignedFadeIn"),
    };
    sites.field(
        prepared,
        &format!("{name}/bits"),
        &word.range,
        payload_label,
    )?;
    sites.converted(
        prepared,
        &format!("{name}/signed_value"),
        &word.range,
        conversion_label,
    )?;
    Ok(Some(SwitchFadeWord {
        bits: word.into(),
        signed_value: i32::from_le_bytes(word.value.to_le_bytes()),
    }))
}
